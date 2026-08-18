use crate::register::legacy_test_support::{IrqDelivery, StatefulIrqHandler, stateful_irq_handler};
use hardware_interface_runtime::SoftwareCpu;
use pdma_model_contract::abi::DmaAddress;
use pdma_model_contract::bridge::{PdmaMemoryRegion, QueueActivation};
use pdma_model_contract::semantic::EpochQueue;
use pdma_model_contract::semantic::{
    BufferId, EpochIrqSource, IrqMessage, IrqMessageId, IrqNotification, IrqRoute, IrqSnapshot,
    IrqSource, IrqVector,
};
use pdma_model_device::{AsyncDevice, AsyncDeviceHandle, BusControlError, PdmaMemoryError};
use pdma_model_host::ModelError;
use pdma_model_host::ModelResult;
use pdma_model_host::{
    AsyncPdmaBus, BusDevicePort, BusQueueStatus, BusSubmission, DriverPort as AsyncDevicePort,
};
use std::{
    future::Future,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};
use tokio::sync::{Mutex, Notify, mpsc, oneshot};

#[derive(Debug, Default)]
struct AllocationGate {
    blocked: AtomicBool,
    entered: AtomicBool,
    released: Notify,
}

#[derive(Clone)]
struct InterleavingBus {
    handle: AsyncDeviceHandle,
    allocation: Arc<AllocationGate>,
    notifications: Arc<Mutex<mpsc::Receiver<IrqNotification>>>,
}

impl InterleavingBus {
    fn new(handle: AsyncDeviceHandle) -> (Self, mpsc::Sender<IrqNotification>) {
        let (sender, receiver) = mpsc::channel(1);
        (
            Self {
                handle,
                allocation: Arc::new(AllocationGate::default()),
                notifications: Arc::new(Mutex::new(receiver)),
            },
            sender,
        )
    }

    fn block_allocation(&self) {
        self.allocation.blocked.store(true, Ordering::Release);
    }

    fn release_allocation(&self) {
        self.allocation.blocked.store(false, Ordering::Release);
        self.allocation.released.notify_waiters();
    }
}

impl AsyncPdmaBus for InterleavingBus {
    type Stage = <AsyncDeviceHandle as AsyncPdmaBus>::Stage;
    type DmaError = PdmaMemoryError;
    type ControlError = BusControlError;

    async fn allocate_memory_region(&self, len: usize) -> ModelResult<PdmaMemoryRegion> {
        if self.allocation.blocked.load(Ordering::Acquire) {
            self.allocation.entered.store(true, Ordering::Release);
            self.allocation.released.notified().await;
        }
        AsyncPdmaBus::allocate_memory_region(&self.handle, len).await
    }

    fn stage_memory_regions(
        &self,
        lengths: &[u64],
    ) -> impl Future<Output = Result<Self::Stage, Self::DmaError>> + Send {
        AsyncPdmaBus::stage_memory_regions(&self.handle, lengths)
    }

    fn read_dma(
        &self,
        address: DmaAddress,
        length: u64,
    ) -> impl Future<Output = Result<Vec<u8>, Self::DmaError>> + Send {
        AsyncPdmaBus::read_dma(&self.handle, address, length)
    }

    fn write_dma(
        &self,
        address: DmaAddress,
        bytes: &[u8],
    ) -> impl Future<Output = Result<(), Self::DmaError>> + Send {
        AsyncPdmaBus::write_dma(&self.handle, address, bytes)
    }

    fn bootstrap_queue(
        &self,
        stage: Self::Stage,
        activation: QueueActivation,
    ) -> impl Future<Output = Result<BusQueueStatus, Self::ControlError>> + Send {
        AsyncPdmaBus::bootstrap_queue(&self.handle, stage, activation)
    }

    fn accept_submission(
        &self,
        request: BusSubmission<Self::Stage>,
    ) -> impl Future<Output = Result<BusQueueStatus, Self::ControlError>> + Send {
        AsyncPdmaBus::accept_submission(&self.handle, request)
    }

    fn read_queue_status(
        &self,
        queue: EpochQueue,
    ) -> impl Future<Output = Result<BusQueueStatus, Self::ControlError>> + Send {
        AsyncPdmaBus::read_queue_status(&self.handle, queue)
    }

    fn write_queue_control(
        &self,
        request: pdma_model_host::QueueControlWrite,
    ) -> impl Future<Output = Result<BusQueueStatus, Self::ControlError>> + Send {
        AsyncPdmaBus::write_queue_control(&self.handle, request)
    }

    fn configure_irq_route(
        &self,
        source: EpochIrqSource,
        route: IrqRoute,
    ) -> impl Future<Output = ModelResult<()>> + Send {
        AsyncPdmaBus::configure_irq_route(&self.handle, source, route)
    }

    fn enable_irq_source(
        &self,
        source: EpochIrqSource,
    ) -> impl Future<Output = ModelResult<()>> + Send {
        AsyncPdmaBus::enable_irq_source(&self.handle, source)
    }

    fn disable_irq_source(
        &self,
        source: EpochIrqSource,
    ) -> impl Future<Output = ModelResult<()>> + Send {
        AsyncPdmaBus::disable_irq_source(&self.handle, source)
    }

    fn mask_irq_source(
        &self,
        source: EpochIrqSource,
    ) -> impl Future<Output = ModelResult<()>> + Send {
        AsyncPdmaBus::mask_irq_source(&self.handle, source)
    }

    fn unmask_irq_source(
        &self,
        source: EpochIrqSource,
    ) -> impl Future<Output = ModelResult<()>> + Send {
        AsyncPdmaBus::unmask_irq_source(&self.handle, source)
    }

    fn ack_irq_source(
        &self,
        source: EpochIrqSource,
    ) -> impl Future<Output = ModelResult<()>> + Send {
        AsyncPdmaBus::ack_irq_source(&self.handle, source)
    }

    fn irq_snapshot(&self) -> impl Future<Output = Option<IrqSnapshot>> + Send {
        AsyncPdmaBus::irq_snapshot(&self.handle)
    }

    fn wait_irq(&self) -> impl Future<Output = Option<IrqSnapshot>> + Send {
        AsyncPdmaBus::wait_irq(&self.handle)
    }

    async fn wait_irq_notification(&self) -> Option<IrqNotification> {
        self.notifications.lock().await.recv().await
    }
}

struct RepollHandler {
    port: BusDevicePort<InterleavingBus>,
    result: Option<oneshot::Sender<ModelResult<BufferId>>>,
}

impl StatefulIrqHandler for RepollHandler {
    async fn handle(&mut self, _notification: IrqNotification) -> ModelResult<()> {
        let result = self.port.allocate_buffer(4).await;
        self.result
            .take()
            .ok_or(ModelError::DmaSubmissionState)?
            .send(result)
            .map_err(|_| ModelError::DmaSubmissionState)
    }
}

/// Verifies a handler on another CPU cannot repoll an owner-pending allocation.
///
/// # Panics
/// Panics when the handler or resumed owner produces an unexpected observation.
pub(crate) async fn verify_handler_turn_rejects_pending_port_repoll() -> ModelResult<()> {
    let (device, _worker) = AsyncDevice::from_core(pdma_model_device::DeviceCore::new(
        std::sync::Arc::new(hardware_interface_runtime::SharedMemory::new()),
        pdma_model_contract::semantic::ResourceConfig::default(),
        std::sync::Arc::new(hardware_interface_runtime::NoopRegisterObserver),
    ));
    let (bus, notification_sender) = InterleavingBus::new(device.handle());
    let owner = SoftwareCpu::new();
    let handler_cpu = SoftwareCpu::new();
    let vector = IrqVector::new(91);
    let (mut main_port, delivery_port) = owner
        .run_main(async {
            let port = BusDevicePort::new(bus.clone())?;
            Ok::<_, ModelError>((port.clone(), port))
        })
        .await?;
    let (result_sender, result_receiver) = oneshot::channel();
    let handler_port = main_port.clone();
    let (delivery, delivery_handle) = IrqDelivery::new(delivery_port, handler_cpu)
        .register_handler(
            vector,
            stateful_irq_handler(RepollHandler {
                port: handler_port,
                result: Some(result_sender),
            }),
        );
    let delivery_task = tokio::spawn(delivery.run());

    bus.block_allocation();
    let main_cpu = owner.clone();
    let main_task = tokio::spawn(async move {
        let result = main_cpu.run_main(main_port.allocate_buffer(4)).await;
        (main_port, result)
    });
    while !bus.allocation.entered.load(Ordering::Acquire) {
        tokio::task::yield_now().await;
    }
    notification_sender
        .send(IrqNotification::Message(IrqMessage::new(
            IrqMessageId::new(0),
            vector,
            EpochIrqSource::new(
                pdma_model_contract::semantic::DeviceEpoch::initial(),
                IrqSource::Completion(pdma_model_contract::semantic::CompletionRingId::new(0)),
            ),
        )))
        .await
        .map_err(|_| ModelError::DmaSubmissionState)?;
    let handler_result = result_receiver
        .await
        .map_err(|_| ModelError::DmaSubmissionState)?;
    assert_eq!(handler_result, Err(ModelError::SoftwareCpuMismatch));
    let next_buffer = BufferId::new(0);
    assert_eq!(device.handle().region_for_object(next_buffer).await, None);

    bus.release_allocation();
    let (_main_port, main_result) = main_task
        .await
        .map_err(|_| ModelError::DmaSubmissionState)?;
    assert_eq!(main_result?.raw(), next_buffer.raw());
    delivery_handle.request_shutdown();
    delivery_task
        .await
        .map_err(|_| ModelError::DmaSubmissionState)??;
    Ok(())
}
use crate::register::legacy_integration_tests::pdma_model_host;
