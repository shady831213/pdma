use crate::interrupt::InterruptError;
use crate::{DriverError, DriverResult, PortError, PortResult};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use crate::irq_delivery::{IrqDelivery, StatefulIrqHandler, stateful_irq_handler};
use crate::{AsyncDriver, DriverPort as AsyncDevicePort, compat::SemanticDevicePort};
use hardware_interface_runtime::SoftwareCpu;
use pdma_model_contract::semantic::{
    BufferId, CompletionRecord, DeviceEpoch, EpochIrqSource, EventRecord, ExternalCommand,
    IrqNotification, IrqRoute, IrqSnapshot, QueueConfig, QueueConfigHandle,
};
use tokio::sync::{Mutex, mpsc};

use super::message;

#[derive(Clone)]
struct ResetPort {
    notifications: Arc<Mutex<mpsc::Receiver<IrqNotification>>>,
    polls: Arc<AtomicUsize>,
    acknowledgements: Arc<AtomicUsize>,
}

impl AsyncDevicePort for ResetPort {
    async fn create_queue(&mut self, _config: QueueConfig) -> PortResult<QueueConfigHandle> {
        std::future::pending().await
    }

    async fn allocate_buffer(&mut self, _len: usize) -> PortResult<BufferId> {
        std::future::pending().await
    }

    async fn write_buffer(
        &mut self,
        _id: BufferId,
        _offset: usize,
        _data: &[u8],
    ) -> PortResult<()> {
        std::future::pending().await
    }

    async fn read_buffer(&self, _id: BufferId, _offset: usize, _len: usize) -> PortResult<Vec<u8>> {
        std::future::pending().await
    }

    async fn submit_command(
        &mut self,
        _queue: QueueConfigHandle,
        _command: ExternalCommand,
    ) -> PortResult<()> {
        std::future::pending().await
    }

    async fn ring_doorbell(&mut self, _queue: QueueConfigHandle) -> PortResult<()> {
        std::future::pending().await
    }

    async fn poll_completion(
        &mut self,
        _queue: QueueConfigHandle,
    ) -> PortResult<Option<CompletionRecord>> {
        self.polls.fetch_add(1, Ordering::SeqCst);
        Err(PortError::StaleEpoch {
            expected: DeviceEpoch::new(1),
            actual: DeviceEpoch::initial(),
        })
    }

    async fn poll_queue_event(
        &mut self,
        _queue: QueueConfigHandle,
    ) -> PortResult<Option<EventRecord>> {
        std::future::pending().await
    }

    async fn configure_irq_route(
        &mut self,
        _source: EpochIrqSource,
        _route: IrqRoute,
    ) -> PortResult<()> {
        std::future::pending().await
    }

    async fn enable_irq_source(&mut self, _source: EpochIrqSource) -> PortResult<()> {
        std::future::pending().await
    }

    async fn disable_irq_source(&mut self, _source: EpochIrqSource) -> PortResult<()> {
        std::future::pending().await
    }

    async fn mask_irq_source(&mut self, _source: EpochIrqSource) -> PortResult<()> {
        std::future::pending().await
    }

    async fn unmask_irq_source(&mut self, _source: EpochIrqSource) -> PortResult<()> {
        std::future::pending().await
    }

    async fn ack_irq_source(&mut self, _source: EpochIrqSource) -> PortResult<()> {
        self.acknowledgements.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }

    async fn irq_snapshot(&self) -> Option<IrqSnapshot> {
        std::future::pending().await
    }

    async fn wait_irq(&self) -> Option<IrqSnapshot> {
        std::future::pending().await
    }

    async fn wait_irq_notification(&mut self) -> Option<IrqNotification> {
        self.notifications.lock().await.recv().await
    }
}

struct ResetHandler {
    driver: AsyncDriver,
    port: ResetPort,
}

impl StatefulIrqHandler for ResetHandler {
    async fn handle(&mut self, notification: IrqNotification) -> DriverResult<()> {
        self.driver
            .service_irq_on(&mut self.port, notification)
            .await
            .map(|_| ())
    }
}

/// Runs the stale post-reset delivery scenario.
///
/// # Panics
/// Panics when stale service reaches acknowledgement or returns the wrong typed error.
pub(crate) async fn run() -> Result<(), Box<dyn std::error::Error>> {
    // Given: an old driver and host-owned notification facing a reset epoch service port.
    let mut device = pdma_model_device::SyncDevice::from_core(pdma_model_device::DeviceCore::new(
        std::sync::Arc::new(hardware_interface_runtime::SharedMemory::new()),
        pdma_model_contract::semantic::ResourceConfig::default(),
        std::sync::Arc::new(hardware_interface_runtime::NoopRegisterObserver),
    ));
    let cpu = SoftwareCpu::new();
    let mut device_port = SemanticDevicePort::new(&mut device);
    let driver = cpu
        .run_main(AsyncDriver::setup_on(&mut device_port, 1, 1))
        .await?;
    let source = driver.completion_irq_source();
    let notification = message(
        0,
        pdma_model_contract::semantic::IrqVector::new(4),
        source.source(),
    );
    let (sender, notifications) = mpsc::channel(1);
    sender
        .send(notification)
        .await
        .map_err(|_| DriverError::QueueFull)?;
    drop(sender);
    let polls = Arc::new(AtomicUsize::new(0));
    let acknowledgements = Arc::new(AtomicUsize::new(0));
    let port = ResetPort {
        notifications: Arc::new(Mutex::new(notifications)),
        polls: Arc::clone(&polls),
        acknowledgements: Arc::clone(&acknowledgements),
    };
    let (delivery, _) = IrqDelivery::new(port.clone(), cpu).register_handler(
        pdma_model_contract::semantic::IrqVector::new(4),
        stateful_irq_handler(ResetHandler { driver, port }),
    );

    // When: the old delivery actor services the stale host-owned notification.
    let result = delivery.run().await;

    // Then: stale queue service terminates delivery before device acknowledgement.
    assert_eq!(
        result,
        Err(InterruptError::Driver(DriverError::Port(
            PortError::StaleEpoch {
                expected: DeviceEpoch::new(1),
                actual: DeviceEpoch::initial(),
            },
        )))
    );
    assert_eq!(polls.load(Ordering::SeqCst), 1);
    assert_eq!(acknowledgements.load(Ordering::SeqCst), 0);
    Ok(())
}
