//! Cancellation preserves committed SG chunks and independent endpoint cursors.

use hardware_interface_pipeline::{AsyncExecutor, Operation};
use hardware_interface_runtime::{
    MemoryAccess, MemoryAccessError, MemoryAddress, MemoryAlignment, SharedMemory,
    SharedMemoryAllocator, SoftwareCpu,
};
use pdma_model_contract::{Command, Endpoint, Response, ResponseStatus, Segment};
use pdma_model_device::{Device, pipeline};
use pdma_model_host::{Channel, RingConfig, Rings, runtime::Model};
use std::{error::Error, sync::Arc};

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

fn scatter_gather(
    allocator: &SharedMemoryAllocator,
    payloads: [&[u8]; 2],
) -> TestResult<(Endpoint, [MemoryAddress; 2])> {
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
        Endpoint::scatter_gather(address.raw(), u64::try_from(segments.len())?),
        [first_address, second_address],
    ))
}

#[tokio::test(flavor = "current_thread")]
async fn committed_sg_chunk_is_not_replayed_when_the_retained_operation_resumes() -> TestResult {
    // Given: source boundaries [3, 5], destination boundaries [2, 6], and one retained operation.
    let allocator =
        SharedMemoryAllocator::new(Arc::new(SharedMemory::new()), MemoryAddress::new(0x1000));
    let command_alignment = MemoryAlignment::try_new(u64::try_from(Command::RECORD_ALIGNMENT)?)?;
    let response_alignment = MemoryAlignment::try_new(u64::try_from(Response::RECORD_ALIGNMENT)?)?;
    let command = allocator.allocate(u64::try_from(Command::RECORD_SIZE)?, command_alignment)?;
    let response = allocator.allocate(u64::try_from(Response::RECORD_SIZE)?, response_alignment)?;
    let rings = Rings {
        command: RingConfig::new(command.raw(), 1)?,
        response: RingConfig::new(response.raw(), 1)?,
    };
    let (source, [source_first, _]) = scatter_gather(&allocator, [b"abc", b"defgh"])?;
    let (destination, [destination_first, destination_second]) =
        scatter_gather(&allocator, [&[0; 2], &[0; 6]])?;
    let memory = allocator.memory_access();
    let model = Model::new(Device::new(memory.clone()));
    let cpu = SoftwareCpu::new();
    let mut channel = cpu
        .run_main(Channel::setup(model.clone(), memory.clone(), rings))
        .await?;
    cpu.run_main(channel.submit(Command {
        id: 31,
        source,
        destination,
    }))
    .await?;
    let executor = AsyncExecutor::new(model.clone());
    let mut stage = pipeline();
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
        result = executor.run(&mut stage, &mut operation) => {
            assert!(result.is_ok());
            false
        },
    };
    assert!(first_chunk_committed);
    assert!(!operation.is_complete());
    assert_eq!(cpu.run_main(channel.response()).await?, None);
    // Test-only mutation detects replay of a committed chunk; SG descriptors stay stable.
    memory.write(destination_first, MemoryAlignment::ONE, b"ZZ")?;
    memory.write(source_first, MemoryAlignment::ONE, b"XX")?;
    assert!(executor.run(&mut stage, &mut operation).await.is_ok());

    // Then: only the remaining logical stream is copied and one completion drains the IRQ.
    let mut first = [0; 2];
    let mut remaining = [0; 6];
    memory.read(destination_first, MemoryAlignment::ONE, &mut first)?;
    memory.read(destination_second, MemoryAlignment::ONE, &mut remaining)?;
    assert_eq!(
        [first.as_slice(), remaining.as_slice()].concat(),
        b"ZZcdefgh"
    );
    assert!(model.irq());
    assert_eq!(
        cpu.run_main(channel.response()).await?,
        Some(Response {
            id: 31,
            status: ResponseStatus::Success,
            bytes_completed: 8,
        })
    );
    assert_eq!(cpu.run_main(channel.response()).await?, None);
    assert!(!model.irq());
    Ok(())
}
