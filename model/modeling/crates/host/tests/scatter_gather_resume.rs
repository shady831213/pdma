//! Cancellation preserves committed SG chunks and independent endpoint cursors.

use pdma_model_contract::dma::{
    Command, CopyCommand, CopyCommandDestinationValues, CopyCommandSourceValues, Event,
    ScatterGatherEndpoint, Segment,
};

use hardware_interface_pipeline::{AsyncExecutor, Operation};
use hardware_interface_runtime::{
    MemoryAccess, MemoryAccessError, MemoryAddress, MemoryAlignment, SharedMemory,
    SharedMemoryAllocator, SoftwareCpu,
};
use pdma_model_dma::Dma;
use pdma_model_host::{Channel, ChannelConfig, RingConfig, Rings, runtime::Model};
use std::{error::Error, sync::Arc};

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

fn scatter_gather(
    allocator: &SharedMemoryAllocator,
    payloads: [&[u8]; 2],
) -> TestResult<(ScatterGatherEndpoint, [MemoryAddress; 2])> {
    let [first, second] = payloads;
    let first_address = allocator.allocate(u64::try_from(first.len())?, MemoryAlignment::ONE)?;
    let second_address = allocator.allocate(u64::try_from(second.len())?, MemoryAlignment::ONE)?;
    let memory = allocator.memory_access();
    memory.write(first_address, MemoryAlignment::ONE, first)?;
    memory.write(second_address, MemoryAlignment::ONE, second)?;
    let segments = [
        Segment {
            address: first_address.raw(),
            length: u64::try_from(first.len())?,
        },
        Segment {
            address: second_address.raw(),
            length: u64::try_from(second.len())?,
        },
    ];
    let bytes = segments
        .iter()
        .copied()
        .map(Segment::to_bytes)
        .collect::<Result<Vec<_>, _>>()?
        .concat();
    let alignment = MemoryAlignment::try_new(u64::try_from(Segment::RECORD_ALIGNMENT)?)?;
    let address = allocator.allocate(u64::try_from(bytes.len())?, alignment)?;
    memory.write(address, alignment, &bytes)?;
    Ok((
        ScatterGatherEndpoint {
            list_address: address.raw(),
            count: u64::try_from(segments.len())?,
            index: 0,
            offset: 0,
        },
        [first_address, second_address],
    ))
}

#[tokio::test(flavor = "current_thread")]
async fn committed_sg_chunk_is_not_replayed_when_the_retained_operation_resumes() -> TestResult {
    // Given: source boundaries [3, 5], destination boundaries [2, 6], and one retained operation.
    let allocator =
        SharedMemoryAllocator::new(Arc::new(SharedMemory::new()), MemoryAddress::new(0x1000));
    let command_alignment = MemoryAlignment::try_new(u64::try_from(Command::RECORD_ALIGNMENT)?)?;
    let event_alignment = MemoryAlignment::try_new(u64::try_from(Event::RECORD_ALIGNMENT)?)?;
    let command = allocator.allocate(u64::try_from(Command::RECORD_SIZE)?, command_alignment)?;
    let event = allocator.allocate(u64::try_from(Event::RECORD_SIZE)?, event_alignment)?;
    let rings = Rings {
        command: RingConfig::new(command.raw(), 1)?,
        event: RingConfig::new(event.raw(), 1)?,
    };
    let (source, [source_first, _]) = scatter_gather(&allocator, [b"abc", b"defgh"])?;
    let (destination, [destination_first, destination_second]) =
        scatter_gather(&allocator, [&[0; 2], &[0; 6]])?;
    let memory = allocator.memory_access();
    let device = Dma::new(memory.clone(), 1)?;
    let mut hardware = device.hardware();
    let model = Model::new(device);
    let cpu = SoftwareCpu::new();
    let mut channel = cpu
        .run_main(Channel::setup(
            model.clone(),
            memory.clone(),
            ChannelConfig {
                id: pdma_model_contract::dma::ChannelId::new(0),
                rings,
            },
        ))
        .await?;
    cpu.run_main(channel.submit(Command::Copy(CopyCommand {
        id: 31,
        copy_length: 8,
        sg_context: 0,
        source: CopyCommandSourceValues::ScatterGather(source),
        destination: CopyCommandDestinationValues::ScatterGather(destination),
        ..pdma_model_contract::dma::CopyCommand::ZERO
    })))
    .await?;
    let executor = AsyncExecutor::new(model.clone());
    let mut operation = Operation::new(());

    // When: cancellation follows the first two-byte write and the same operation resumes.
    let first_chunk_committed = tokio::select! {
        biased;
        observed = async {
            for _ in 0..256 {
                let mut first = [0; 2];
                let mut remaining = [0; 6];
                memory.read(destination_first, MemoryAlignment::ONE, &mut first)?;
                memory.read(destination_second, MemoryAlignment::ONE, &mut remaining)?;
                if &first == b"ab" && remaining == [0; 6] {
                    return Ok::<bool, MemoryAccessError>(true);
                }
                tokio::task::yield_now().await;
            }
            Ok(false)
        } => observed?,
        result = executor.run(&mut hardware, &mut operation) => {
            return Err(format!("hardware terminated: {result:?}").into());
        },
    };
    assert!(first_chunk_committed);
    assert!(!operation.is_complete());
    assert_eq!(cpu.run_main(channel.event()).await?, None);
    // Test-only mutation detects replay of a committed chunk; SG descriptors stay stable.
    memory.write(destination_first, MemoryAlignment::ONE, b"ZZ")?;
    memory.write(source_first, MemoryAlignment::ONE, b"XX")?;
    assert!(executor.run(&mut hardware, &mut operation).await.is_ok());
    assert!(operation.is_complete());

    // Then: only the remaining stream is copied, without replay or an error event.
    let mut first = [0; 2];
    let mut remaining = [0; 6];
    memory.read(destination_first, MemoryAlignment::ONE, &mut first)?;
    memory.read(destination_second, MemoryAlignment::ONE, &mut remaining)?;
    assert_eq!(
        [first.as_slice(), remaining.as_slice()].concat(),
        b"ZZcdefgh"
    );
    assert_eq!(cpu.run_main(channel.event()).await?, None);
    assert!(!model.irq());
    Ok(())
}
