#![allow(
    dead_code,
    reason = "shared helpers are compiled separately by each integration test"
)]
#![allow(
    clippy::expect_used,
    reason = "checked fixture constructors use literals proven valid by separate tests"
)]
#![allow(
    unreachable_pub,
    reason = "integration test support is public only within each private support module"
)]

use hw_model_io::memory::MemoryAlignment;
use hw_model_io::memory::{MemoryRegion, SharedMemory};

pub fn alignment(value: u64) -> MemoryAlignment {
    MemoryAlignment::try_new(value).expect("valid alignment")
}

pub fn region(base: u64, length: u64, alignment_value: u64) -> MemoryRegion {
    MemoryRegion::try_new(base.into(), length, alignment(alignment_value)).expect("valid region")
}

pub fn install(memory: &SharedMemory, regions: &[MemoryRegion]) {
    memory.add_regions(regions).expect("regions install");
}
