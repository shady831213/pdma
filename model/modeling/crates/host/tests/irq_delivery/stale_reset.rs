use pdma_model_host::ModelError;
use pdma_model_host::ModelResult;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use pdma_model_contract::{
    BufferId, CompletionRecord, DeviceEpoch, EpochIrqSource, EventRecord, ExternalCommand,
    IrqNotification, IrqRoute, IrqSnapshot, QueueConfig, QueueConfigHandle,
};
use pdma_model_host::{
    AsyncDevicePort, AsyncDriver, IrqDelivery, SoftwareCpu, StatefulIrqHandler,
    stateful_irq_handler,
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
    async fn create_queue(&mut self, _config: QueueConfig) -> ModelResult<QueueConfigHandle> {
        std::future::pending().await
    }

    async fn allocate_buffer(&mut self, _len: usize) -> ModelResult<BufferId> {
        std::future::pending().await
    }

    async fn write_buffer(
        &mut self,
        _id: BufferId,
        _offset: usize,
        _data: &[u8],
    ) -> ModelResult<()> {
        std::future::pending().await
    }

    async fn read_buffer(
        &self,
        _id: BufferId,
        _offset: usize,
        _len: usize,
    ) -> ModelResult<Vec<u8>> {
        std::future::pending().await
    }

    async fn submit_command(
        &mut self,
        _queue: QueueConfigHandle,
        _command: ExternalCommand,
    ) -> ModelResult<()> {
        std::future::pending().await
    }

    async fn ring_doorbell(&mut self, _queue: QueueConfigHandle) -> ModelResult<()> {
        std::future::pending().await
    }

    async fn poll_completion(
        &mut self,
        _queue: QueueConfigHandle,
    ) -> ModelResult<Option<CompletionRecord>> {
        self.polls.fetch_add(1, Ordering::SeqCst);
        Err(ModelError::StaleDeviceEpoch {
            expected: DeviceEpoch::new(1),
            actual: DeviceEpoch::initial(),
        })
    }

    async fn poll_queue_event(
        &mut self,
        _queue: QueueConfigHandle,
    ) -> ModelResult<Option<EventRecord>> {
        std::future::pending().await
    }

    async fn configure_irq_route(
        &mut self,
        _source: EpochIrqSource,
        _route: IrqRoute,
    ) -> ModelResult<()> {
        std::future::pending().await
    }

    async fn enable_irq_source(&mut self, _source: EpochIrqSource) -> ModelResult<()> {
        std::future::pending().await
    }

    async fn disable_irq_source(&mut self, _source: EpochIrqSource) -> ModelResult<()> {
        std::future::pending().await
    }

    async fn mask_irq_source(&mut self, _source: EpochIrqSource) -> ModelResult<()> {
        std::future::pending().await
    }

    async fn unmask_irq_source(&mut self, _source: EpochIrqSource) -> ModelResult<()> {
        std::future::pending().await
    }

    async fn ack_irq_source(&mut self, _source: EpochIrqSource) -> ModelResult<()> {
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
    async fn handle(&mut self, notification: IrqNotification) -> ModelResult<()> {
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
pub async fn run() -> ModelResult<()> {
    // Given: an old driver and host-owned notification facing a reset epoch service port.
    let mut device = pdma_model_device::SyncDevice::from_core(pdma_model_device::DeviceCore::new(
        std::sync::Arc::new(hardware_interface_runtime::SharedMemory::new()),
        pdma_model_contract::ResourceConfig::default(),
        std::sync::Arc::new(hardware_interface_runtime::NoopRegisterObserver),
    ));
    let cpu = SoftwareCpu::new();
    let driver = cpu
        .run_main(AsyncDriver::setup_on(&mut device, 1, 1))
        .await?;
    let source = driver.completion_irq_source();
    let notification = message(0, pdma_model_contract::IrqVector::new(4), source.source());
    let (sender, notifications) = mpsc::channel(1);
    sender
        .send(notification)
        .await
        .map_err(|_| ModelError::DriverQueueFull)?;
    drop(sender);
    let polls = Arc::new(AtomicUsize::new(0));
    let acknowledgements = Arc::new(AtomicUsize::new(0));
    let port = ResetPort {
        notifications: Arc::new(Mutex::new(notifications)),
        polls: Arc::clone(&polls),
        acknowledgements: Arc::clone(&acknowledgements),
    };
    let (delivery, _) = IrqDelivery::new(port.clone(), cpu).register_handler(
        pdma_model_contract::IrqVector::new(4),
        stateful_irq_handler(ResetHandler { driver, port }),
    );

    // When: the old delivery actor services the stale host-owned notification.
    let result = delivery.run().await;

    // Then: stale queue service terminates delivery before device acknowledgement.
    assert_eq!(
        result,
        Err(ModelError::StaleDeviceEpoch {
            expected: DeviceEpoch::new(1),
            actual: DeviceEpoch::initial(),
        })
    );
    assert_eq!(polls.load(Ordering::SeqCst), 1);
    assert_eq!(acknowledgements.load(Ordering::SeqCst), 0);
    Ok(())
}
