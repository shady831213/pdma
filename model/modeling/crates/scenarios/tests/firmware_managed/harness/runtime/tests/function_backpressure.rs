use super::{client, control::rings, *};
use hardware_interface_runtime::{
    MemoryAccess, MemoryAddress, MemoryAlignment, SharedMemoryAllocator,
};
use pdma_model_contract::{ChannelId, ControlQueue, ControlRingState};
use pdma_model_host::{Management, ManagementConfig};
use std::time::Duration;

#[tokio::test]
async fn response_full_function_does_not_block_peer_requests() {
    // Given: A fills its two-response ring and deliberately leaves both results uncollected.
    let allocator =
        SharedMemoryAllocator::new(Arc::new(SharedMemory::new()), MemoryAddress::new(0x1000));
    let (system, mut hosts) = Managed::with_functions(allocator.memory_access(), 3, 2).unwrap();
    let b = hosts.pop().unwrap();
    let a = hosts.pop().unwrap();
    let (full, wait_full) = tokio::sync::oneshot::channel();
    let (done, wait_done) = tokio::sync::oneshot::channel();
    let a = Box::pin(client::run(a.runtime, async |interrupt| {
        let storage = rings(&allocator).unwrap();
        let memory = allocator.memory_access();
        let mut management = Management::setup(
            a.bus.clone(),
            memory.clone(),
            ManagementConfig {
                addresses: a.bus.addresses(),
                rings: storage,
                interrupt,
            },
        )
        .await
        .unwrap();
        management.submit_acquire().await.unwrap();
        management.submit_acquire().await.unwrap();
        let producer = MemoryAddress::new(
            storage.state.raw()
                + u64::try_from(ControlQueue::RESPONSE_OFFSET + ControlRingState::PRODUCER_OFFSET)
                    .unwrap(),
        );
        loop {
            let bytes: [u8; 4] = memory.read_typed(producer, MemoryAlignment::ONE).unwrap();
            if u32::from_le_bytes(bytes) == 2 {
                break;
            }
            tokio::task::yield_now().await;
        }
        full.send(()).unwrap();
        wait_done.await.unwrap();
        assert!(management.try_response().await.unwrap().is_some());
    }));
    let b = Box::pin(client::run(b.runtime, async |interrupt| {
        let mut management = Management::setup(
            b.bus.clone(),
            allocator.memory_access(),
            ManagementConfig {
                addresses: b.bus.addresses(),
                rings: rings(&allocator).unwrap(),
                interrupt,
            },
        )
        .await
        .unwrap();
        wait_full.await.unwrap();
        // When: B asks for the remaining channel while A provides no response space.
        let granted = management.acquire_channel().await.unwrap();
        // Then: B completes independently; A did not have to consume either response.
        assert_eq!(granted, ChannelId::new(2));
        management.release_channel(granted).await.unwrap();
        done.send(()).unwrap();
    }));
    tokio::time::timeout(
        Duration::from_secs(5),
        system.run_until(async {
            tokio::join!(a, b);
        }),
    )
    .await
    .unwrap()
    .unwrap();
}
