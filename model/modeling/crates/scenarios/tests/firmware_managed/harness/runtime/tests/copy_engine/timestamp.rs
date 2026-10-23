use super::{
    Managed, Management, Ports, SharedMemory, SharedMemoryAllocator, client, program, rings,
};
use pdma_model_contract::memory::FunctionId;
use std::{sync::Arc, time::Duration};

#[tokio::test]
async fn managed_timestamp_observes_copy_order_and_survives_channel_reset() {
    // Given: platform hardware and firmware; software owns its queues and all result storage.
    let allocator = SharedMemoryAllocator::new(Arc::new(SharedMemory::new()), 0x1000.into());
    let (system, host) = Managed::with_channels(allocator.memory_access(), 2).unwrap();
    let program = client::run(host.runtime, async |interrupt| {
        let mut management = Management::setup(
            host.bus.clone(),
            allocator.memory_access(),
            client::config(rings(&allocator).unwrap(), &interrupt),
        )
        .await
        .unwrap();
        let owner = FunctionId::new(0);
        let channels = [
            management.acquire_channel(owner).await.unwrap(),
            management.acquire_channel(owner).await.unwrap(),
        ];
        let payload = [0x5a; 16];
        // When: Host submits timestamp/copy/timestamp/fence/trap, resets, and submits fresh work.
        let first = program::timestamp::run(
            Ports {
                bus: host.bus.clone(),
                allocator: allocator.clone(),
                channels,
            },
            &payload,
        )
        .await
        .unwrap();
        for id in channels {
            management.reset_channel(owner, id).await.unwrap();
        }
        let second = program::timestamp::run(
            Ports {
                bus: host.bus,
                allocator,
                channels,
            },
            &payload,
        )
        .await
        .unwrap();
        for id in channels {
            management.release_channel(owner, id).await.unwrap();
        }
        (first, second)
    });
    let (first, second) =
        tokio::time::timeout(Duration::from_secs(3), system.run_until(Box::pin(program)))
            .await
            .unwrap()
            .unwrap();
    // Then: ordered execution has ordered samples, and channel reset did not reset platform time.
    assert!(first[0] < first[1]);
    assert!(first[1] < second[0]);
    assert!(second[0] < second[1]);
}
