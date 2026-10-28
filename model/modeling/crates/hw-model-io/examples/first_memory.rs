//! First byte transfer with local or shared backing.
//!
//! Run: `cargo run -p hw-model-io --example first_memory`.
//! The backing owner installs regions; consumers receive only access capability.
use hw_model_io::memory::{LocalMemory, MemoryRegion, SharedMemory};
use hw_model_io::memory::{MemoryAccess, MemoryAlignment, SharedMemoryAccess};
use std::sync::Arc;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let local = LocalMemory::new();
    let shared = SharedMemory::new();
    // These are independent stores. Installing the same region does not alias bytes.
    let region = MemoryRegion::try_new(0.into(), 4, MemoryAlignment::ONE)?;
    local.add_regions(&[region])?;
    shared.add_regions(&[region])?;
    // Buffer access is byte-oriented. The application chooses scalar byte order;
    // a four-byte payload does not require four-byte alignment for ordinary access.
    let bytes = 42_u32.to_le_bytes();
    local.write_buffer(0.into(), MemoryAlignment::ONE, &bytes)?;
    shared.write_buffer(0.into(), MemoryAlignment::ONE, &bytes)?;
    // Give a consumer byte-access authority without region-management methods.
    let access = SharedMemoryAccess::from(Arc::new(shared));
    let observed = access.read_buffer::<[u8; 4]>(0.into(), MemoryAlignment::ONE)?;
    assert_eq!(u32::from_le_bytes(observed), 42);
    assert_eq!(
        local.read_buffer::<[u8; 4]>(0.into(), MemoryAlignment::ONE)?,
        observed
    );
    println!("local/shared byte buffers decoded explicitly: 42");
    Ok(())
}
