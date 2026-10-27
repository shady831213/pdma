#![forbid(unsafe_code)]
#![doc = include_str!("../README.md")]

mod access;
mod address;
mod allocator;
mod atomic;
mod bound_memory;
mod error;
mod region;
mod shared_access;
mod store;

pub use access::{MemoryAccess, MemoryAccessError};
pub use address::{MemoryAddress, MemoryAlignment};
pub use allocator::{MemoryAllocationError, SharedMemoryAllocator};
pub use atomic::{
    AtomicAccessError, AtomicOperation, AtomicOrder, AtomicReadOrder, AtomicUpdate,
    AtomicWriteOrder,
};
pub use bound_memory::BoundMemory;
pub use error::{MemoryAlignmentError, MemoryError, MemoryRegionError};
use hardware_interface_storage::{LocalStorage, MutexStorage, StorageBackend, StorageError};
pub use region::MemoryRegion;
pub use shared_access::SharedMemoryAccess;
pub use store::{LocalMemoryStorage, Memory, MemoryStorage, SharedMemory, SharedMemoryStorage};
