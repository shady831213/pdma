use super::{client, control::rings, *};
use hardware_interface_runtime::{MemoryAddress, SharedMemoryAllocator};
use pdma_model_contract::{ChannelId, ControlResponseStatus};
use pdma_model_host::{Management, ManagementConfig};
use std::time::Duration;

#[tokio::test]
async fn dormant_function_does_not_block_another_functions_control_queue() {
    // Given: Function zero never enables its control ring.
    let allocator =
        SharedMemoryAllocator::new(Arc::new(SharedMemory::new()), MemoryAddress::new(0x1000));
    let (system, mut hosts) = Managed::with_functions(allocator.memory_access(), 1, 2).unwrap();
    let b = hosts.pop().unwrap();
    // When: Function one initializes and requests the only DMA channel.
    let program = client::run(b.runtime, async |interrupt| {
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
        let channel = management.acquire_channel().await.unwrap();
        management.release_channel(channel).await.unwrap();
        channel
    });
    let channel = tokio::time::timeout(Duration::from_secs(5), system.run_until(program))
        .await
        .unwrap()
        .unwrap();
    // Then: no initialization or notification from Function zero was needed.
    assert_eq!(channel, ChannelId::new(0));
}

#[tokio::test]
async fn independent_functions_cannot_reset_release_or_access_peer_channels() {
    // Given: two independent Host capabilities over one DMA and firmware.
    let allocator =
        SharedMemoryAllocator::new(Arc::new(SharedMemory::new()), MemoryAddress::new(0x1000));
    let (system, mut hosts) = Managed::with_functions(allocator.memory_access(), 2, 2).unwrap();
    let b = hosts.pop().unwrap();
    let a = hosts.pop().unwrap();
    let (send_a, receive_a) = tokio::sync::oneshot::channel();
    let (send_b, receive_b) = tokio::sync::oneshot::channel();
    let (released, wait_release) = tokio::sync::oneshot::channel();
    let (rebound, wait_rebound) = tokio::sync::oneshot::channel();
    let program_a = Box::pin(client::run(a.runtime, async |interrupt| {
        let mut management = Management::setup(
            a.bus.clone(),
            allocator.memory_access(),
            ManagementConfig {
                addresses: a.bus.addresses(),
                rings: rings(&allocator).unwrap(),
                interrupt,
            },
        )
        .await
        .unwrap();
        let mine = management.acquire_channel().await.unwrap();
        send_a.send(mine).unwrap();
        let peer: ChannelId = receive_b.await.unwrap();
        // When: A names B's real channel in lifecycle requests and MMIO accesses.
        management.submit_reset(peer).await.unwrap();
        assert_eq!(
            management.next_response().await.unwrap().status,
            ControlResponseStatus::PermissionDenied
        );
        management.submit_release(peer).await.unwrap();
        assert_eq!(
            management.next_response().await.unwrap().status,
            ControlResponseStatus::PermissionDenied
        );
        let peer_address = u32::try_from(peer.register_offset(2).unwrap()).unwrap();
        assert!(matches!(
            a.bus.read(peer_address, &mut [0; 8]).await,
            Err(AccessError::Denied)
        ));
        assert!(matches!(
            a.bus.write(peer_address, &[0; 8]).await,
            Err(AccessError::Denied)
        ));
        management.reset_channel(mine).await.unwrap();
        management.release_channel(mine).await.unwrap();
        released.send(()).unwrap();
        wait_rebound.await.unwrap();
        // Then: reassignment does not restore the old Function's access.
        let old_address = u32::try_from(mine.register_offset(2).unwrap()).unwrap();
        assert!(matches!(
            a.bus.read(old_address, &mut [0; 8]).await,
            Err(AccessError::Denied)
        ));
    }));
    let program_b = Box::pin(client::run(b.runtime, async |interrupt| {
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
        let mine = management.acquire_channel().await.unwrap();
        send_b.send(mine).unwrap();
        let peer = receive_a.await.unwrap();
        wait_release.await.unwrap();
        let mine_address = u32::try_from(mine.register_offset(2).unwrap()).unwrap();
        b.bus.read(mine_address, &mut [0; 8]).await.unwrap();
        assert_eq!(management.acquire_channel().await.unwrap(), peer);
        rebound.send(()).unwrap();
    }));
    tokio::time::timeout(
        Duration::from_secs(5),
        system.run_until(async {
            tokio::join!(program_a, program_b);
        }),
    )
    .await
    .unwrap()
    .unwrap();
}
