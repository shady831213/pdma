//! Exact device-side ABI state and behavior.

pub mod ring;

pub use ring::{
    DmaBatchContext, DmaWrite, PdmaBatchError, PdmaControlError, PendingDmaResult,
    QueueBootstrapBatch, QueueResponseBytes, QueueStatus, SubmissionAcceptance, SubmissionBatch,
};
