use hardware_interface_io::memory::MemoryAccessError;
use pdma_model_contract::dma::{
    Command, CommandRegisters, ContextId, CopyCommandDestinationValues, CopyCommandSourceValues,
    Event, LinearEndpoint,
};

use super::*;
use hardware_interface_io::memory::MemoryAccess;
use pdma_model_contract::memory::{
    ContextAttributes, ContextAttributesAddressSpace, MemoryIdentity,
};
use pdma_model_host::{Channel, ChannelConfig, Rings};

#[tokio::test]
async fn pending_drain_and_reset_complete_with_independent_results() {
    // Given: real firmware and DMA share a live, long-running payload context.
    let allocator = SharedMemoryAllocator::new(Arc::new(SharedMemory::new()), 0x1000.into());
    let memory = allocator.memory_access();
    let pf = FunctionId::new(0);
    let payload_attributes = ContextAttributes {
        address_space: ContextAttributesAddressSpace::Pasid,
        pasid: 19,
    };
    let spaces = [ContextAttributes::DEFAULT, payload_attributes]
        .into_iter()
        .map(|attributes| {
            (
                MemoryIdentity {
                    function: pf,
                    attributes,
                },
                memory.clone(),
            )
        })
        .collect();
    let (system, mut hosts) = Managed::with_context_memory(
        spaces,
        1,
        super::super::super::platform::ContextPlatform {
            function_count: 1,
            capacity: 2,
        },
    )
    .unwrap();
    let host = hosts.pop().unwrap();
    let program = client::run(host.runtime, async |interrupt| {
        let control_rings = rings(&allocator).unwrap();
        let mut management = Management::setup(
            host.bus.clone(),
            memory.clone(),
            client::config(control_rings, &interrupt),
        )
        .await
        .unwrap();
        let mut contexts = Vec::new();
        for attributes in [ContextAttributes::DEFAULT, payload_attributes] {
            management
                .submit_create_context(pf, attributes)
                .await
                .unwrap();
            let response = management.next_response().await.unwrap();
            assert_eq!(response.status, ControlResponseStatus::Success);
            contexts.push(ContextId::new(response.context));
        }
        let payload = *contexts.last().unwrap();
        let id = management.acquire_channel(pf).await.unwrap();
        let (destination, source_bytes) = submit_copy(host.bus.clone(), &allocator, (id, payload))
            .await
            .unwrap();
        // When: PF submits reset without waiting for its pending Drain response.
        let drain = management.submit_drain_context(pf, payload).await.unwrap();
        let reset = management.submit_reset(pf, id).await.unwrap();
        let mut results = [
            management.next_response().await.unwrap(),
            management.next_response().await.unwrap(),
        ];
        results.sort_by_key(|result| result.id);
        // Then: drain fails, reset succeeds, and the old command cannot write again.
        assert_eq!(
            results.map(|result| (result.id, result.status)),
            [
                (drain.raw(), ControlResponseStatus::MaintenanceFailed),
                (reset.raw(), ControlResponseStatus::Success)
            ]
        );
        let mut before = vec![0; source_bytes.len()];
        memory
            .read(destination, MemoryAlignment::ONE, &mut before)
            .unwrap();
        assert_ne!(before, source_bytes);
        for _ in 0..64 {
            tokio::task::yield_now().await;
        }
        let mut after = vec![0; source_bytes.len()];
        memory
            .read(destination, MemoryAlignment::ONE, &mut after)
            .unwrap();
        assert_eq!(after, before);
        management
            .submit_destroy_context(pf, payload)
            .await
            .unwrap();
        assert_eq!(
            management.next_response().await.unwrap().status,
            ControlResponseStatus::Success
        );
    });
    tokio::time::timeout(Duration::from_secs(5), system.run_until(Box::pin(program)))
        .await
        .unwrap()
        .unwrap();
}

async fn submit_copy(
    bus: crate::harness::runtime::HostBus<SharedMemoryAccess<(), MemoryAccessError>>,
    allocator: &SharedMemoryAllocator,
    (id, payload): (ChannelId, ContextId),
) -> crate::harness::TestResult<(MemoryAddress, Vec<u8>)> {
    let memory = allocator.memory_access();
    let source_bytes = vec![0x6d; 65_536];
    let source = allocator.allocate(u64::try_from(source_bytes.len())?, MemoryAlignment::ONE)?;
    let destination =
        allocator.allocate(u64::try_from(source_bytes.len())?, MemoryAlignment::ONE)?;
    memory.write(source, MemoryAlignment::ONE, &source_bytes)?;
    let mut channel = Channel::setup(
        bus.clone(),
        memory.clone(),
        ChannelConfig {
            id,
            rings: Rings {
                command: ring::<Command>(allocator)?,
                event: ring::<Event>(allocator)?,
            },
        },
    )
    .await?;
    channel
        .submit(Command::Copy(pdma_model_contract::dma::CopyCommand {
            id: 71,
            sg_context: payload.raw(),
            source: CopyCommandSourceValues::Linear(LinearEndpoint {
                address: source.raw(),
                offset: 0,
            }),
            source_context: payload.raw(),
            destination: CopyCommandDestinationValues::Linear(LinearEndpoint {
                address: destination.raw(),
                offset: 0,
            }),
            destination_context: payload.raw(),
            copy_length: u64::try_from(source_bytes.len())?,
            ..pdma_model_contract::dma::CopyCommand::ZERO
        }))
        .await?;
    let consumer = u32::try_from(
        id.register_offset(1).ok_or("channel layout")? + CommandRegisters::CONSUMER.offset(),
    )?;
    loop {
        let mut bytes = [0; 4];
        bus.read(consumer, &mut bytes).await?;
        if u32::from_le_bytes(bytes) == 1 {
            break;
        }
        tokio::task::yield_now().await;
    }
    Ok((destination, source_bytes))
}
