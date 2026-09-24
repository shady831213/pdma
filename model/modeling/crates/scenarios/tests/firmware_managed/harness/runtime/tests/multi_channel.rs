use super::{client, control::rings, *};
use hardware_interface_runtime::{MemoryAddress, SharedMemoryAllocator};
use pdma_model_contract::control::ControlResponseStatus;
use pdma_model_contract::dma::ChannelId;
use pdma_model_contract::memory::FunctionId;
use pdma_model_host::Management;
use std::time::Duration;

#[tokio::test]
async fn outstanding_acquisitions_and_release_preserve_peer_permissions() {
    // Given: three closed channels and one trusted Host with two-slot management rings.
    let allocator =
        SharedMemoryAllocator::new(Arc::new(SharedMemory::new()), MemoryAddress::new(0x1000));
    let (system, host) = Managed::with_channels(allocator.memory_access(), 3).unwrap();
    let program = client::run(host.runtime, async |interrupt| {
        let mut management = Management::setup(
            host.bus.clone(),
            allocator.memory_access(),
            client::config(rings(&allocator).unwrap(), &interrupt),
        )
        .await
        .unwrap();
        // When: two acquisitions are published before either result is collected.
        let first = management.submit_acquire(FunctionId::new(0)).await.unwrap();
        let second = management.submit_acquire(FunctionId::new(0)).await.unwrap();
        let a = management.next_response().await.unwrap();
        let b = management.next_response().await.unwrap();
        // Then: correlation and independently enforced apertures survive release of one grant.
        assert_eq!(
            (a.id, a.status, a.channel),
            (first.raw(), ControlResponseStatus::Success, 0)
        );
        assert_eq!(
            (b.id, b.status, b.channel),
            (second.raw(), ControlResponseStatus::Success, 1)
        );
        let mut bytes = [0; 8];
        let address =
            |index| u32::try_from(ChannelId::new(index).register_offset(3).unwrap()).unwrap();
        host.bus.read(address(0), &mut bytes).await.unwrap();
        host.bus.read(address(1), &mut bytes).await.unwrap();
        assert!(matches!(
            host.bus.read(address(2), &mut bytes).await,
            Err(AccessError::Closed)
        ));
        management
            .release_channel(FunctionId::new(0), ChannelId::new(0))
            .await
            .unwrap();
        assert!(matches!(
            host.bus.read(address(0), &mut bytes).await,
            Err(AccessError::Closed)
        ));
        host.bus.read(address(1), &mut bytes).await.unwrap();
        assert_eq!(
            management
                .acquire_channel(FunctionId::new(0))
                .await
                .unwrap(),
            ChannelId::new(0)
        );
    });
    tokio::time::timeout(Duration::from_secs(3), system.run_until(Box::pin(program)))
        .await
        .unwrap()
        .unwrap();
}
