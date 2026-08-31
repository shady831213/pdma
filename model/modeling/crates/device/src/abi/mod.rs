//! Exact device-side ABI state and behavior.

pub mod dma;
pub mod endpoint;
pub mod mmio;
pub mod ring;

pub use endpoint::{MmioRegisterEndpoint, MmioRegisterError};
pub use ring::{
    DmaBatchContext, DmaWrite, PdmaBatchError, PdmaControlError, PendingDmaResult,
    QueueBootstrapBatch, QueueResponseBytes, QueueStatus, SubmissionAcceptance, SubmissionBatch,
};
