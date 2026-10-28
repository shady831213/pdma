#![doc = include_str!("memory/README.md")]

mod access;
mod address;
mod allocator;
mod atomic;
mod bound_memory;
mod error;
mod region;
mod shared_access;
/// Closure-scoped local and shared storage for arbitrary values.
pub mod storage;
mod store;

use storage::{LocalStorage, MutexStorage, StorageBackend, StorageError};

pub use access::MemoryAccess;
pub use address::{MemoryAddress, MemoryAlignment, MemoryAlignmentError};
pub use allocator::{MemoryAllocationError, SharedMemoryAllocator};
pub use atomic::{
    AtomicAccessError, AtomicOperation, AtomicOrder, AtomicReadOrder, AtomicUpdate,
    AtomicWriteOrder,
};
pub use bound_memory::BoundMemory;
pub use error::{MemoryAccessError, MemoryError, MemoryRegionError};
pub use region::MemoryRegion;
pub use shared_access::SharedMemoryAccess;
pub use store::{
    LocalMemory, LocalMemoryPolicy, Memory, MemoryPolicy, SharedMemory, SharedMemoryPolicy,
};
