use super::{client, control::rings, *};
use hw_model_io::memory::MemoryAddress;
use hw_model_io::memory::SharedMemoryAllocator;
use pdma_model_contract::control::ControlResponseStatus;
use pdma_model_contract::dma::ChannelId;
use pdma_model_contract::memory::FunctionId;
use pdma_model_host::{Management, ManagementConfig};
use std::time::Duration;

#[tokio::test]
async fn pf_manages_dormant_vfs_and_rejects_invalid_target_functions() {
    // Given: only PF software boots, with two dormant VF ports.
    let allocator = SharedMemoryAllocator::new(
        Arc::new(SharedMemory::new()),
        MemoryAddress::new(0x1000),
        64 * 1024,
    )
    .expect("valid test heap");
    let (system, mut hosts) = Managed::with_functions(allocator.memory_access(), 1, 3).unwrap();
    let _b = hosts.pop().unwrap();
    let a = hosts.pop().unwrap();
    let pf = hosts.pop().unwrap();
    let program = client::run(pf.runtime, async |interrupt| {
        let mut management = Management::setup(
            pf.bus.clone(),
            allocator.memory_access(),
            ManagementConfig {
                addresses: pf.bus.addresses(),
                rings: rings(&allocator).unwrap(),
                interrupt: interrupt.response,
                event_interrupt: interrupt.event,
            },
        )
        .await
        .unwrap();
        // When: PF submits out-of-range targets before a valid VF request.
        for target in [3, u32::MAX] {
            management
                .submit_acquire(FunctionId::new(target))
                .await
                .unwrap();
            assert_eq!(
                management.next_response().await.unwrap().status,
                ControlResponseStatus::InvalidFunction
            );
            management
                .submit_reset(FunctionId::new(target), ChannelId::new(0))
                .await
                .unwrap();
            assert_eq!(
                management.next_response().await.unwrap().status,
                ControlResponseStatus::InvalidFunction
            );
            management
                .submit_release(FunctionId::new(target), ChannelId::new(0))
                .await
                .unwrap();
            assert_eq!(
                management.next_response().await.unwrap().status,
                ControlResponseStatus::InvalidFunction
            );
        }
        // Then: no resource was consumed; the dormant VF owns the only channel.
        let channel = management
            .acquire_channel(FunctionId::new(1))
            .await
            .unwrap();
        assert_eq!(channel, ChannelId::new(0));
        let address = u32::try_from(channel.register_offset(1).unwrap()).unwrap();
        a.bus.read(address, &mut [0; 8]).await.unwrap();
        assert!(matches!(
            pf.bus.read(address, &mut [0; 8]).await,
            Err(AccessError::Denied)
        ));
        management
            .release_channel(FunctionId::new(1), channel)
            .await
            .unwrap();
        assert!(matches!(
            a.bus.read(address, &mut [0; 8]).await,
            Err(AccessError::Closed)
        ));
    });
    tokio::time::timeout(Duration::from_secs(5), system.run_until(Box::pin(program)))
        .await
        .unwrap()
        .unwrap();
}

#[tokio::test]
async fn pf_lifecycle_validates_target_ownership_and_reassignment_revokes_old_access() {
    // Given: one PF management endpoint and two independently bound VF work ports.
    let allocator = SharedMemoryAllocator::new(
        Arc::new(SharedMemory::new()),
        MemoryAddress::new(0x1000),
        64 * 1024,
    )
    .expect("valid test heap");
    let (system, mut hosts) = Managed::with_functions(allocator.memory_access(), 2, 3).unwrap();
    let b = hosts.pop().unwrap();
    let a = hosts.pop().unwrap();
    let pf = hosts.pop().unwrap();
    let fa = FunctionId::new(1);
    let fb = FunctionId::new(2);
    let program = client::run(pf.runtime, async |interrupt| {
        let mut management = Management::setup(
            pf.bus.clone(),
            allocator.memory_access(),
            ManagementConfig {
                addresses: pf.bus.addresses(),
                rings: rings(&allocator).unwrap(),
                interrupt: interrupt.response,
                event_interrupt: interrupt.event,
            },
        )
        .await
        .unwrap();
        let first = management.submit_acquire(fa).await.unwrap();
        let second = management.submit_acquire(fb).await.unwrap();
        let a_response = management.next_response().await.unwrap();
        let b_response = management.next_response().await.unwrap();
        assert_eq!(
            (a_response.id, a_response.status),
            (first.raw(), ControlResponseStatus::Success)
        );
        assert_eq!(
            (b_response.id, b_response.status),
            (second.raw(), ControlResponseStatus::Success)
        );
        let mine = ChannelId::new(usize::try_from(a_response.channel).unwrap());
        let peer = ChannelId::new(usize::try_from(b_response.channel).unwrap());
        let mine_address = u32::try_from(mine.register_offset(2).unwrap()).unwrap();
        let peer_address = u32::try_from(peer.register_offset(2).unwrap()).unwrap();
        // When: PF supplies the wrong target Function for a real peer channel.
        management.submit_reset(fa, peer).await.unwrap();
        assert_eq!(
            management.next_response().await.unwrap().status,
            ControlResponseStatus::PermissionDenied
        );
        management.submit_release(fa, peer).await.unwrap();
        assert_eq!(
            management.next_response().await.unwrap().status,
            ControlResponseStatus::PermissionDenied
        );
        // Then: ownership remains intact and neither VF can access its peer.
        a.bus.read(mine_address, &mut [0; 8]).await.unwrap();
        b.bus.read(peer_address, &mut [0; 8]).await.unwrap();
        for (bus, address) in [
            (&a.bus, peer_address),
            (&b.bus, mine_address),
            (&pf.bus, mine_address),
        ] {
            assert!(matches!(
                bus.read(address, &mut [0; 8]).await,
                Err(AccessError::Denied)
            ));
            assert!(matches!(
                bus.write(address, &[0; 8]).await,
                Err(AccessError::Denied)
            ));
        }
        management.reset_channel(fa, mine).await.unwrap();
        a.bus.read(mine_address, &mut [0; 8]).await.unwrap();
        b.bus.read(peer_address, &mut [0; 8]).await.unwrap();
        management.release_channel(fa, mine).await.unwrap();
        assert!(matches!(
            a.bus.read(mine_address, &mut [0; 8]).await,
            Err(AccessError::Closed)
        ));
        assert_eq!(management.acquire_channel(fb).await.unwrap(), mine);
        b.bus.read(mine_address, &mut [0; 8]).await.unwrap();
        assert!(matches!(
            a.bus.read(mine_address, &mut [0; 8]).await,
            Err(AccessError::Denied)
        ));
        management.release_channel(fb, mine).await.unwrap();
        management.release_channel(fb, peer).await.unwrap();
    });
    tokio::time::timeout(Duration::from_secs(5), system.run_until(Box::pin(program)))
        .await
        .unwrap()
        .unwrap();
}
