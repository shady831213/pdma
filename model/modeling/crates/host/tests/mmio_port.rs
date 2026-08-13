//! MMIO-backed host-port CPU, reset, and DMA-persistence boundaries.

use pdma_model_host::ModelError;
use pdma_model_host::ModelResult;

use pdma_model_contract::abi::mmio::{MmioByteCount, MmioResult};
use pdma_model_contract::{
    CopyRequest, EventRouting, IrqNotification, IrqRoute, IrqVector, QueueConfig, SoftwareOwnerId,
};
use pdma_model_device::AsyncDevice;
use pdma_model_host::{
    AsyncDevicePort, AsyncDriver, AsyncPdmaBus, BusDevicePort, MmioDeviceBus, SoftwareCpu,
};

#[tokio::test(flavor = "current_thread")]
async fn message_delivery_uses_non_consuming_readiness_and_one_raw_claim() -> ModelResult<()> {
    // Given: the real worker and one Message-routed completion through an MMIO port.
    let (device, worker) = AsyncDevice::from_core(pdma_model_device::DeviceCore::new(
        std::sync::Arc::new(hardware_interface_runtime::SharedMemory::new()),
        pdma_model_contract::ResourceConfig::default(),
        std::sync::Arc::new(hardware_interface_runtime::NoopRegisterObserver),
    ));
    let worker = tokio::spawn(worker.run());
    let bus = MmioDeviceBus::new(device.handle());
    let cpu = SoftwareCpu::new();
    let source = cpu
        .run_main(async {
            let mut port = BusDevicePort::new(bus.clone())?;
            let mut driver = AsyncDriver::setup_on(&mut port, 2, 2).await?;
            let input = driver.allocate_buffer_on(&mut port, 4).await?;
            let output = driver.allocate_buffer_on(&mut port, 4).await?;
            driver
                .write_buffer_on(&mut port, input, &[1, 2, 3, 4])
                .await?;
            let source = driver.completion_irq_source();
            port.configure_irq_route(source, IrqRoute::Message(IrqVector::new(41)))
                .await?;
            port.enable_irq_source(source).await?;
            port.unmask_irq_source(source).await?;
            driver
                .submit_copy_on(&mut port, CopyRequest::new(input, output, 4))
                .await?;
            Ok::<_, ModelError>(source)
        })
        .await?;

    // When: host delivery waits for readiness and claims through the raw IRQ window.
    let notification = bus
        .wait_irq_notification()
        .await
        .ok_or(ModelError::IrqDeliveryUnavailable)?;

    // Then: the one claimed Message retains its exact source, vector, and first identity.
    let IrqNotification::Message(message) = notification else {
        return Err(ModelError::IrqDeliveryUnavailable);
    };
    assert_eq!(message.source_capability(), source);
    assert_eq!(message.vector(), IrqVector::new(41));
    assert_eq!(message.id().raw(), 0);
    device.request_shutdown().await;
    worker.await.map_err(|_| ModelError::DmaSubmissionState)?;
    Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn owner_cpu_and_reset_boundaries_hold_through_mmio_port() -> ModelResult<()> {
    // Given: one owner-bound MMIO port, a queue capability, and persistent DMA bytes.
    let (device, _worker) = AsyncDevice::from_core(pdma_model_device::DeviceCore::new(
        std::sync::Arc::new(hardware_interface_runtime::SharedMemory::new()),
        pdma_model_contract::ResourceConfig::default(),
        std::sync::Arc::new(hardware_interface_runtime::NoopRegisterObserver),
    ));
    let handle = device.handle();
    let bus = MmioDeviceBus::new(handle.clone());
    let owner = SoftwareCpu::new();
    let wrong = SoftwareCpu::new();

    // When: a wrong nested CPU attempts allocation before owner work and whole-device reset.
    owner
        .run_main(async {
            let mut port = BusDevicePort::new(bus)?;
            let mut wrong_port = port.clone();
            let rejected = wrong.run_main(wrong_port.allocate_buffer(4)).await;
            assert_eq!(rejected, Err(ModelError::SoftwareCpuMismatch));
            let buffer = port.allocate_buffer(4).await?;
            assert_eq!(buffer.raw(), 0);
            port.write_buffer(buffer, 0, &[1, 2, 3, 4]).await?;
            let queue = port
                .create_queue(QueueConfig::new(
                    2,
                    2,
                    EventRouting::QueueLocal,
                    SoftwareOwnerId::new(7),
                ))
                .await?;
            let source = queue.completion_irq_source();
            port.configure_irq_route(source, IrqRoute::Message(IrqVector::new(41)))
                .await?;
            handle
                .write_mmio(0x018, &0_u64.to_le_bytes())
                .await
                .map_err(|_| ModelError::DmaSubmissionState)?;
            handle
                .write_mmio(0x020, &4_u32.to_le_bytes())
                .await
                .map_err(|_| ModelError::DmaSubmissionState)?;
            let result = handle
                .read_mmio(0x024, MmioByteCount::from(4_u8))
                .await
                .map_err(|_| ModelError::DmaSubmissionState)?;

            // Then: reset is exact, old capabilities are stale, and committed DMA persists.
            assert_eq!(result, MmioResult::Ok.code().to_le_bytes());
            assert!(matches!(
                port.enable_irq_source(source).await,
                Err(ModelError::StaleDeviceEpoch { .. })
            ));
            assert_eq!(port.read_buffer(buffer, 0, 4).await?, vec![1, 2, 3, 4]);
            Ok(())
        })
        .await
}

#[tokio::test(flavor = "current_thread")]
async fn inactive_cpu_rejects_before_mmio_port_mutation() -> ModelResult<()> {
    // Given: a port returned from its sole active owner segment.
    let (device, _worker) = AsyncDevice::from_core(pdma_model_device::DeviceCore::new(
        std::sync::Arc::new(hardware_interface_runtime::SharedMemory::new()),
        pdma_model_contract::ResourceConfig::default(),
        std::sync::Arc::new(hardware_interface_runtime::NoopRegisterObserver),
    ));
    let owner = SoftwareCpu::new();
    let mut port = owner
        .run_main(async { BusDevicePort::new(MmioDeviceBus::new(device.handle())) })
        .await?;

    // When: allocation is polled with no active software CPU.
    let rejected = port.allocate_buffer(4).await;

    // Then: per-poll preflight rejects before the DMA allocator commit point.
    assert_eq!(rejected, Err(ModelError::SoftwareCpuInactive));
    Ok(())
}
