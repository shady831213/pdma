//! Exact device-side ABI state and behavior.

pub mod endpoint;
pub mod mmio;
pub mod ring;

pub use endpoint::MmioRegisterError;
pub use ring::{
    DmaBatchContext, DmaWrite, PdmaBatchError, PdmaControlError, PendingDmaResult,
    QueueBootstrapBatch, QueueResponseBytes, QueueStatus, SubmissionAcceptance, SubmissionBatch,
};
