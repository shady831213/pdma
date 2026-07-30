use super::ConsumerLane;
use pdma_model_contract::abi::{DmaAddress, DmaRegion, EpochQueue, QueueActivation};
use pdma_model_contract::{EpochIrqSource, IrqNotification, IrqRoute, IrqSnapshot, ModelResult};
use pdma_model_device::{AsyncDeviceHandle, BusControlError, DmaSpaceError};
use pdma_model_host::{AsyncPdmaBus, BusQueueStatus, BusSubmission, QueueControlWrite};
use std::{
    future::{Future, poll_fn},
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    task::Poll,
};
use tokio::sync::{Mutex, Notify, mpsc};

#[derive(Debug)]
struct ConsumerGate {
    lane: ConsumerLane,
    armed: AtomicBool,
    entered: AtomicBool,
    released: AtomicBool,
    entered_notify: Notify,
    release_notify: Arc<Notify>,
    matching_polls: AtomicUsize,
    delegated_writes: AtomicUsize,
}

impl ConsumerGate {
    fn new(lane: ConsumerLane) -> Self {
        Self {
            lane,
            armed: AtomicBool::new(false),
            entered: AtomicBool::new(false),
            released: AtomicBool::new(false),
            entered_notify: Notify::new(),
            release_notify: Arc::new(Notify::new()),
            matching_polls: AtomicUsize::new(0),
            delegated_writes: AtomicUsize::new(0),
        }
    }
}

#[derive(Clone)]
pub(super) struct GatedBus {
    handle: AsyncDeviceHandle,
    gate: Arc<ConsumerGate>,
    notifications: Arc<Mutex<mpsc::Receiver<IrqNotification>>>,
}

impl GatedBus {
    pub(super) fn new(
        handle: AsyncDeviceHandle,
        lane: ConsumerLane,
    ) -> (Self, mpsc::Sender<IrqNotification>) {
        let (sender, receiver) = mpsc::channel(1);
        (
            Self {
                handle,
                gate: Arc::new(ConsumerGate::new(lane)),
                notifications: Arc::new(Mutex::new(receiver)),
            },
            sender,
        )
    }

    pub(super) fn arm(&self) {
        self.gate.armed.store(true, Ordering::Release);
    }

    pub(super) async fn wait_entered(&self) {
        loop {
            let entered = self.gate.entered_notify.notified();
            if self.gate.entered.load(Ordering::Acquire) {
                return;
            }
            entered.await;
        }
    }

    pub(super) fn release(&self) {
        self.gate.released.store(true, Ordering::Release);
        self.gate.release_notify.notify_waiters();
    }

    pub(super) fn counts(&self) -> (usize, usize) {
        (
            self.gate.matching_polls.load(Ordering::Acquire),
            self.gate.delegated_writes.load(Ordering::Acquire),
        )
    }
}

impl AsyncPdmaBus for GatedBus {
    type Stage = <AsyncDeviceHandle as AsyncPdmaBus>::Stage;
    type DmaError = DmaSpaceError;
    type ControlError = BusControlError;

    fn allocate_dma_region(
        &self,
        len: usize,
    ) -> impl Future<Output = ModelResult<DmaRegion>> + Send {
        AsyncPdmaBus::allocate_dma_region(&self.handle, len)
    }

    fn stage_dma_regions(
        &self,
        lengths: &[u64],
    ) -> impl Future<Output = Result<Self::Stage, Self::DmaError>> + Send {
        AsyncPdmaBus::stage_dma_regions(&self.handle, lengths)
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
        request: QueueControlWrite,
    ) -> impl Future<Output = Result<BusQueueStatus, Self::ControlError>> + Send {
        let gate = Arc::clone(&self.gate);
        let handle = self.handle.clone();
        async move {
            let matching = gate.lane.matches(request);
            let mut waiting = false;
            let mut delegated = false;
            let mut release = Box::pin(Arc::clone(&gate.release_notify).notified_owned());
            let mut raw = Box::pin(AsyncPdmaBus::write_queue_control(&handle, request));
            poll_fn(move |context| {
                if matching {
                    gate.matching_polls.fetch_add(1, Ordering::AcqRel);
                    if !waiting && gate.armed.swap(false, Ordering::AcqRel) {
                        waiting = true;
                        gate.entered.store(true, Ordering::Release);
                        gate.entered_notify.notify_waiters();
                    }
                    if waiting
                        && !gate.released.load(Ordering::Acquire)
                        && release.as_mut().poll(context).is_pending()
                        && !gate.released.load(Ordering::Acquire)
                    {
                        return Poll::Pending;
                    }
                    if !delegated {
                        gate.delegated_writes.fetch_add(1, Ordering::AcqRel);
                        delegated = true;
                    }
                }
                raw.as_mut().poll(context)
            })
            .await
        }
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
