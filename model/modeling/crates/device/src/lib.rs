//! Async PDMA device model runner.

pub mod compat;
pub mod dma_space;
mod error;
mod runner;
pub mod trace;

pub use dma_space::{PdmaMemoryError, PdmaMemoryMappings, PdmaMemoryStage};
pub use error::{ModelError, ModelResult, PdmaResetError, PdmaRunError};
pub use hardware_interface_runtime::{
    NoopRegisterObserver, RegisterObservation, RegisterObserver, SharedMemory,
    SharedRegisterObserver,
};
pub use runner::{
    AsyncDevice, AsyncDeviceHandle, AsyncDeviceWorker, AsyncRunSnapshot, AsyncRunSummary,
    AsyncStallReason, AsyncStepStatus, BusControlError, DeviceCore, DeviceStateSnapshot,
    DmaBatchContext, DmaWrite, MmioBatchError, MmioRegisterEndpoint, MmioRegisterError,
    PdmaBatchError, PdmaControlError, PendingDmaResult, QueueBootstrapBatch, QueueResponseBytes,
    QueueStatus, ResetPartitionSnapshot, SubmissionAcceptance, SubmissionBatch, SyncDevice,
};
pub use trace::TraceSink;
