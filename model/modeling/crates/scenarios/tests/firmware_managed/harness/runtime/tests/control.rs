use super::client;
use super::*;
use hardware_interface_io::memory::SharedMemoryAllocator;
use hardware_interface_io::memory::{MemoryAddress, MemoryAlignment};
use pdma_model_contract::control::{
    ControlQueue, ControlRequest, ControlResponse, ControlResponseStatus,
};
use pdma_model_contract::dma::ChannelId;
use pdma_model_contract::memory::FunctionId;
use pdma_model_contract::ring::Ring;
use pdma_model_host::{Error, Management, ManagementRings, RingConfig, firmware};
use std::{future::Future, pin::pin, task::Poll, time::Duration};

mod in_flight;
mod maintenance_reset;
pub(super) mod memory;
mod raw;

pub(super) fn rings(
    allocator: &SharedMemoryAllocator,
) -> crate::harness::TestResult<ManagementRings> {
    Ok(ManagementRings {
        state: allocator.allocate(
            u64::try_from(ControlQueue::RECORD_SIZE)?,
            MemoryAlignment::try_new(u64::try_from(ControlQueue::RECORD_ALIGNMENT)?)?,
        )?,
        command: ring::<ControlRequest>(allocator)?,
        response: ring::<ControlResponse>(allocator)?,
        event: ring::<pdma_model_contract::control::ControlEvent>(allocator)?,
    })
}

pub(super) fn ring<E: hardware_interface_schema::SchemaCodec>(
    allocator: &SharedMemoryAllocator,
) -> crate::harness::TestResult<RingConfig<E>> {
    let layout = RingConfig::<E>::new(0, 2)?;
    let base = allocator.allocate(
        layout
            .entry_stride()?
            .checked_mul(2)
            .ok_or("ring extent overflow")?,
        MemoryAlignment::try_new(u64::try_from(E::ALIGNMENT)?)?,
    )?;
    Ok(RingConfig::new(base.raw(), 2)?)
}

#[tokio::test]
async fn host_acquires_releases_and_reuses_channel_through_control_rings() {
    // Given: separate firmware and Host CPUs over one real managed channel.
    let allocator =
        SharedMemoryAllocator::new(Arc::new(SharedMemory::new()), MemoryAddress::new(0x1000));
    let (system, host) = Managed::new(allocator.memory_access()).unwrap();
    // When: Host uses the independent management rings while the DMA gate starts closed.
    let program = client::run(host.runtime, async |interrupt| {
        firmware::wait_ready(
            &host.bus,
            host.bus.addresses().firmware,
            Duration::from_secs(1),
        )
        .await
        .unwrap();
        let mut word = [0; 4];
        host.bus.read(WINDOW_STATUS_BASE, &mut word).await.unwrap();
        assert_eq!(word, [0; 4]);
        let mut management = Management::setup(
            host.bus.clone(),
            allocator.memory_access(),
            client::config(rings(&allocator).unwrap(), &interrupt),
        )
        .await
        .unwrap();
        for _ in 0..3 {
            let channel = management
                .acquire_channel(FunctionId::new(0))
                .await
                .unwrap();
            assert_eq!(channel, ChannelId::new(0));
            assert!(matches!(
                management.acquire_channel(FunctionId::new(0)).await,
                Err(Error::ManagementStatus(ControlResponseStatus::NoResources))
            ));
            management
                .release_channel(FunctionId::new(0), channel)
                .await
                .unwrap();
            assert!(matches!(
                host.bus
                    .read(
                        u32::try_from(pdma_model_contract::dma::ChannelBlock::ALIGNMENT).unwrap(),
                        &mut word
                    )
                    .await,
                Err(AccessError::Closed)
            ));
        }
        // Then: invalid releases are responses, not firmware crashes or opened windows.
        assert!(matches!(
            management
                .release_channel(FunctionId::new(0), ChannelId::new(7))
                .await,
            Err(Error::ManagementStatus(
                ControlResponseStatus::InvalidChannel
            ))
        ));
        assert!(matches!(
            management
                .release_channel(FunctionId::new(0), ChannelId::new(0))
                .await,
            Err(Error::ManagementStatus(ControlResponseStatus::NotAllocated))
        ));
    });
    tokio::time::timeout(Duration::from_secs(3), system.run_until(Box::pin(program)))
        .await
        .unwrap()
        .unwrap();
}

#[tokio::test]
async fn cancelling_host_wait_retains_one_request_and_resumes_its_response() {
    // Given: firmware booted once and is temporarily not being polled.
    let allocator =
        SharedMemoryAllocator::new(Arc::new(SharedMemory::new()), MemoryAddress::new(0x1000));
    let (system, host) = Managed::new(allocator.memory_access()).unwrap();
    let mut running = pin!(system.run());
    assert!(
        std::future::poll_fn(|cx| Poll::Ready(running.as_mut().poll(cx)))
            .await
            .is_pending()
    );
    client::run(host.runtime, async |interrupt| {
        let mut management = Management::setup(
            host.bus.clone(),
            allocator.memory_access(),
            client::config(rings(&allocator).unwrap(), &interrupt),
        )
        .await
        .unwrap();
        {
            let mut request = pin!(management.acquire_channel(FunctionId::new(0)));
            assert!(
                std::future::poll_fn(|cx| Poll::Ready(request.as_mut().poll(cx)))
                    .await
                    .is_pending()
            );
            // When: the Host drops only its wait after publishing the command.
        }
        // Then: another request is rejected; driving firmware completes the retained one.
        assert!(matches!(
            management.acquire_channel(FunctionId::new(0)).await,
            Err(Error::ManagementBusy)
        ));
        assert!(
            std::future::poll_fn(|cx| Poll::Ready(running.as_mut().poll(cx)))
                .await
                .is_pending()
        );
        let response = management.wait_pending().await.unwrap();
        assert_eq!(response.id, 1);
        assert_eq!(response.status, ControlResponseStatus::Success);
        assert_eq!(response.channel, 0);
    })
    .await;
}
