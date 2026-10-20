//! Shared software signal operations over the real memory backend.

mod errors;
mod operations;
mod waits;

use hardware_interface_runtime::{MemoryAlignment, MemoryRegion, SharedMemory};

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn memory() -> Result<SharedMemory, Box<dyn std::error::Error>> {
    let memory = SharedMemory::new();
    memory.add_regions(&[MemoryRegion::try_new(
        0x1000.into(),
        16,
        MemoryAlignment::try_new(8)?,
    )?])?;
    Ok(memory)
}
