//! Transport-neutral asynchronous PDMA bus contract.

mod async_device;
mod mmio_device;
mod stage;

pub use mmio_device::{MmioBusControlError, MmioDeviceBus, MmioDeviceTransport};
pub use stage::{AsyncDeviceBusStage, BusDmaStage, BusDmaWrite};

use pdma_model_contract::abi::mmio::IrqSourceStatus;
use pdma_model_contract::abi::{
    DmaAddress, DmaRegion, EpochQueue, QueueActivation, QueueControl, QueueCursorUpdate,
    RingCursor, RingCursors,
};
use pdma_model_contract::{EpochIrqSource, IrqNotification, IrqRoute, IrqSnapshot, ModelResult};
use std::error::Error;

/// One all-or-none raw submission acceptance request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BusSubmission<S> {
    queue: EpochQueue,
    expected_producer: RingCursor,
    descriptor: BusDmaWrite,
    indirect: Option<(S, BusDmaWrite)>,
}

impl<S> BusSubmission<S> {
    /// Groups an optional staged list, opaque writes, and producer commit.
    pub const fn new(
        queue: EpochQueue,
        expected_producer: RingCursor,
        descriptor: BusDmaWrite,
        indirect: Option<(S, BusDmaWrite)>,
    ) -> Self {
        Self {
            queue,
            expected_producer,
            descriptor,
            indirect,
        }
    }

    /// Epoch-qualified target queue.
    pub const fn queue(&self) -> EpochQueue {
        self.queue
    }

    /// Producer value that must still be current.
    pub const fn expected_producer(&self) -> RingCursor {
        self.expected_producer
    }

    /// Opaque submission descriptor write.
    pub const fn descriptor(&self) -> &BusDmaWrite {
        &self.descriptor
    }

    /// Optional staged region and opaque indirect-list write.
    pub const fn indirect(&self) -> Option<(&S, &BusDmaWrite)> {
        match &self.indirect {
            Some((stage, write)) => Some((stage, write)),
            None => None,
        }
    }

    fn into_parts(
        self,
    ) -> (
        EpochQueue,
        RingCursor,
        BusDmaWrite,
        Option<(S, BusDmaWrite)>,
    ) {
        (
            self.queue,
            self.expected_producer,
            self.descriptor,
            self.indirect,
        )
    }
}

/// Typed queue-control write without the read-status variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueueControlWrite {
    /// Capture the accepted submission producer at a doorbell.
    CaptureSubmissionProducer(QueueCursorUpdate),
    /// Publish device submission consumption.
    PublishSubmissionConsumer(QueueCursorUpdate),
    /// Publish device completion production.
    PublishCompletionProducer(QueueCursorUpdate),
    /// Publish host completion consumption.
    PublishCompletionConsumer(QueueCursorUpdate),
    /// Publish device event production.
    PublishEventProducer(QueueCursorUpdate),
    /// Publish host event consumption.
    PublishEventConsumer(QueueCursorUpdate),
}

impl QueueControlWrite {
    /// Epoch-qualified target queue.
    pub const fn queue(self) -> EpochQueue {
        match self {
            Self::CaptureSubmissionProducer(update)
            | Self::PublishSubmissionConsumer(update)
            | Self::PublishCompletionProducer(update)
            | Self::PublishCompletionConsumer(update)
            | Self::PublishEventProducer(update)
            | Self::PublishEventConsumer(update) => update.queue(),
        }
    }

    const fn into_control(self) -> QueueControl {
        match self {
            Self::CaptureSubmissionProducer(update) => {
                QueueControl::CaptureSubmissionProducer(update)
            }
            Self::PublishSubmissionConsumer(update) => {
                QueueControl::PublishSubmissionConsumer(update)
            }
            Self::PublishCompletionProducer(update) => {
                QueueControl::PublishCompletionProducer(update)
            }
            Self::PublishCompletionConsumer(update) => {
                QueueControl::PublishCompletionConsumer(update)
            }
            Self::PublishEventProducer(update) => QueueControl::PublishEventProducer(update),
            Self::PublishEventConsumer(update) => QueueControl::PublishEventConsumer(update),
        }
    }
}

/// Complete typed shadow state returned by raw queue control.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BusQueueStatus {
    queue: EpochQueue,
    submission: RingCursors,
    captured_submission_producer: RingCursor,
    completion: RingCursors,
    event: RingCursors,
}

impl BusQueueStatus {
    /// Creates a status from typed queue shadows.
    pub const fn new(
        queue: EpochQueue,
        submission: RingCursors,
        captured_submission_producer: RingCursor,
        completion: RingCursors,
        event: RingCursors,
    ) -> Self {
        Self {
            queue,
            submission,
            captured_submission_producer,
            completion,
            event,
        }
    }

    /// Epoch-qualified queue identity.
    pub const fn queue(self) -> EpochQueue {
        self.queue
    }

    /// Submission producer and consumer.
    pub const fn submission(self) -> RingCursors {
        self.submission
    }

    /// Submission producer captured by the last doorbell.
    pub const fn captured_submission_producer(self) -> RingCursor {
        self.captured_submission_producer
    }

    /// Completion producer and consumer.
    pub const fn completion(self) -> RingCursors {
        self.completion
    }

    /// Event producer and consumer.
    pub const fn event(self) -> RingCursors {
        self.event
    }
}

/// Raw asynchronous transport contract used to drive one PDMA device.
pub trait AsyncPdmaBus: Send + Sync {
    /// Transport-owned read-only DMA stage capability.
    type Stage: BusDmaStage;
    /// Typed DMA staging or access error.
    type DmaError: Error + Send + Sync + 'static;
    /// Typed atomic control error.
    type ControlError: Error + Send + Sync + 'static;

    /// Allocates and returns one committed raw DMA region.
    fn allocate_dma_region(
        &self,
        len: usize,
    ) -> impl Future<Output = ModelResult<DmaRegion>> + Send;

    /// Proposes DMA regions without committing allocator or mapping state.
    fn stage_dma_regions(
        &self,
        lengths: &[u64],
    ) -> impl Future<Output = Result<Self::Stage, Self::DmaError>> + Send;

    /// Reads one checked IOVA extent.
    fn read_dma(
        &self,
        address: DmaAddress,
        length: u64,
    ) -> impl Future<Output = Result<Vec<u8>, Self::DmaError>> + Send;

    /// Writes one checked IOVA extent.
    fn write_dma(
        &self,
        address: DmaAddress,
        bytes: &[u8],
    ) -> impl Future<Output = Result<(), Self::DmaError>> + Send;

    /// Atomically commits staged rings together with queue activation.
    fn bootstrap_queue(
        &self,
        stage: Self::Stage,
        activation: QueueActivation,
    ) -> impl Future<Output = Result<BusQueueStatus, Self::ControlError>> + Send;

    /// Atomically commits an optional stage, opaque writes, and producer acceptance.
    fn accept_submission(
        &self,
        request: BusSubmission<Self::Stage>,
    ) -> impl Future<Output = Result<BusQueueStatus, Self::ControlError>> + Send;

    /// Reads complete typed queue shadow state.
    fn read_queue_status(
        &self,
        queue: EpochQueue,
    ) -> impl Future<Output = Result<BusQueueStatus, Self::ControlError>> + Send;

    /// Commits one typed queue control write.
    fn write_queue_control(
        &self,
        request: QueueControlWrite,
    ) -> impl Future<Output = Result<BusQueueStatus, Self::ControlError>> + Send;

    /// Configures one inactive PDMA-local IRQ source.
    fn configure_irq_route(
        &self,
        _source: EpochIrqSource,
        _route: IrqRoute,
    ) -> impl Future<Output = ModelResult<()>> + Send {
        async { Err(pdma_model_contract::ModelError::DmaSubmissionState) }
    }

    /// Enables one PDMA-local IRQ source.
    fn enable_irq_source(
        &self,
        _source: EpochIrqSource,
    ) -> impl Future<Output = ModelResult<()>> + Send {
        async { Err(pdma_model_contract::ModelError::DmaSubmissionState) }
    }

    /// Disables one PDMA-local IRQ source.
    fn disable_irq_source(
        &self,
        _source: EpochIrqSource,
    ) -> impl Future<Output = ModelResult<()>> + Send {
        async { Err(pdma_model_contract::ModelError::DmaSubmissionState) }
    }

    /// Masks one PDMA-local IRQ source.
    fn mask_irq_source(
        &self,
        _source: EpochIrqSource,
    ) -> impl Future<Output = ModelResult<()>> + Send {
        async { Err(pdma_model_contract::ModelError::DmaSubmissionState) }
    }

    /// Unmasks one PDMA-local IRQ source.
    fn unmask_irq_source(
        &self,
        _source: EpochIrqSource,
    ) -> impl Future<Output = ModelResult<()>> + Send {
        async { Err(pdma_model_contract::ModelError::DmaSubmissionState) }
    }

    /// Acknowledges one PDMA-local IRQ source.
    fn ack_irq_source(
        &self,
        _source: EpochIrqSource,
    ) -> impl Future<Output = ModelResult<()>> + Send {
        async { Err(pdma_model_contract::ModelError::DmaSubmissionState) }
    }

    /// Observes one PDMA-local IRQ source without consuming it.
    fn observe_irq_source(
        &self,
        _source: EpochIrqSource,
    ) -> impl Future<Output = ModelResult<IrqSourceStatus>> + Send {
        async { Err(pdma_model_contract::ModelError::DmaSubmissionState) }
    }

    /// Returns the current asserted line snapshot.
    fn irq_snapshot(&self) -> impl Future<Output = Option<IrqSnapshot>> + Send {
        async { None }
    }

    /// Waits for one asserted line snapshot.
    fn wait_irq(&self) -> impl Future<Output = Option<IrqSnapshot>> + Send {
        async { None }
    }

    /// Waits for one epoch-bearing line or message notification.
    fn wait_irq_notification(&self) -> impl Future<Output = Option<IrqNotification>> + Send;
}
