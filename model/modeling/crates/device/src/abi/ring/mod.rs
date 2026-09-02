//! Exact ABI queue activation, publication, and batch transactions.

/// Public batch request and failure types.
pub mod batch {
    pub use crate::{
        DmaBatchContext, PdmaBatchError, QueueBootstrapBatch, QueueResponseBytes, SubmissionBatch,
    };

    #[doc(hidden)]
    pub mod commit {}
}
pub use crate::queue::exact::control;
pub use crate::queue::exact::{
    DmaWrite, PdmaControlError, PendingDmaResult, QueueStatus, SubmissionAcceptance,
};
pub use batch::{
    DmaBatchContext, PdmaBatchError, QueueBootstrapBatch, QueueResponseBytes, SubmissionBatch,
};
