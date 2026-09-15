#![forbid(unsafe_code)]
#![doc = include_str!("../README.md")]

mod access;
mod address;
mod allocator;
mod cpu;
mod error;
mod interrupt;
mod region;
mod register;
mod shared_access;
mod storage;
mod store;
mod trace;

pub use access::{MemoryAccess, MemoryAccessError};
pub use address::{MemoryAddress, MemoryAlignment};
pub use allocator::{MemoryAllocationError, SharedMemoryAllocator};
pub use cpu::{CpuContextError, CpuLocal, CpuOwner, SoftwareCpu};
pub use error::{MemoryAlignmentError, MemoryError, MemoryRegionError};
pub use interrupt::{
    DuplicateHandler, EmptyRegistry, FrozenHandlerRegistry, HandlerRegistry,
    InterruptDispatchError, InterruptDispatcher, InterruptDispatcherTask, InterruptEnvelope,
    InterruptHandler, InterruptJoinError, InterruptLifecycleEvent, InterruptLifecycleObserver,
    InterruptRegistration, InterruptRegistrationError, InterruptRuntime, InterruptShutdown,
    InterruptShutdownReceiver, InterruptSource, InterruptSummary, NoopInterruptObserver,
    SharedInterruptObserver,
};
pub use region::MemoryRegion;
pub use register::{
    AccessKind, AccessOrigin, AddressedRegisterAccess, BlockingObservationBackend,
    BlockingObservationReservation, GateFault, LocalObservationBackend,
    LocalObservationReservation, NoopRegisterObserver, ObservationBackend,
    ObservationEmissionReceipt, ObservationGate, ObservationGateError, ObservationOrder,
    ObservationOrderError, ObservationReservation, ObservationReservationReceipt, RegisterAccess,
    RegisterBus, RegisterBusAddress, RegisterBusBase, RegisterBusError, RegisterBusOffset,
    RegisterBusWidth, RegisterEndpointError, RegisterImage, RegisterImageError,
    RegisterObservation, RegisterObserver, SharedRegisterObserver,
};
pub use shared_access::SharedMemoryAccess;
pub use storage::{LocalStorage, MutexStorage, StorageBackend, StorageError};
pub use store::{LocalMemoryStorage, Memory, MemoryStorage, SharedMemory, SharedMemoryStorage};
pub use trace::TraceBuffer;
