#![forbid(unsafe_code)]
#![doc = include_str!("../README.md")]

mod access;
mod address;
mod cpu;
mod error;
mod interrupt;
mod region;
mod register;
mod storage;
mod store;
mod trace;

pub use access::{MemoryAccess, MemoryAccessError};
pub use address::{MemoryAddress, MemoryAlignment, MemoryRevision};
pub use cpu::{CpuContextError, CpuLocal, CpuOwner, SoftwareCpu};
pub use error::{MemoryAlignmentError, MemoryError, MemoryRegionError};
pub use interrupt::{
    DuplicateHandler, EmptyRegistry, FrozenHandlerRegistry, HandlerRegistry,
    InterruptDispatchError, InterruptDispatcher, InterruptDispatcherTask, InterruptEnvelope,
    InterruptHandler, InterruptJoinError, InterruptLifecycleEvent, InterruptLifecycleObserver,
    InterruptShutdown, InterruptShutdownReceiver, InterruptSource, InterruptSummary,
    NoopInterruptObserver, SharedInterruptObserver,
};
pub use region::MemoryRegion;
pub use register::{
    AccessKind, AccessOrigin, AddressedRegisterAccess, GateFault, NoopRegisterObserver,
    ObservationBackend, ObservationEmissionReceipt, ObservationGate, ObservationGateError,
    ObservationOrder, ObservationOrderError, ObservationReservation, ObservationReservationReceipt,
    RegisterAccess, RegisterBusAddress, RegisterBusBase, RegisterBusError, RegisterBusOffset,
    RegisterBusWidth, RegisterObservation, RegisterObserver, SharedRegisterObserver,
};
pub use storage::{LocalStorage, MutexStorage, StorageBackend, StorageError};
pub use store::{
    LocalMemoryStorage, Memory, MemoryStorage, MemoryTransaction, MemoryTransactionDraft,
    PreparedRegions, SharedMemory, SharedMemoryStorage, TransactionPreparedRegions,
};
pub use trace::TraceBuffer;
