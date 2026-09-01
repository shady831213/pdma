//! Exact ABI queue activation, publication, and batch transactions.

pub mod batch;
pub use crate::queue::exact::control;
pub use crate::queue::exact::{
    DmaWrite, PdmaControlError, PendingDmaResult, QueueStatus, SubmissionAcceptance,
};
#[cfg(test)]
pub(crate) use batch::BatchPreparationFault;

pub use batch::{
    DmaBatchContext, PdmaBatchError, QueueBootstrapBatch, QueueResponseBytes, SubmissionBatch,
};
