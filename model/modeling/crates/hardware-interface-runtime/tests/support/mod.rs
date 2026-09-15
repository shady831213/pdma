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

use hardware_interface_runtime::{MemoryAlignment, MemoryRegion, SharedMemory};
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll, Waker};

pub fn alignment(value: u64) -> MemoryAlignment {
    MemoryAlignment::try_new(value).expect("valid alignment")
}

pub fn region(base: u64, length: u64, alignment_value: u64) -> MemoryRegion {
    MemoryRegion::try_new(base.into(), length, alignment(alignment_value)).expect("valid region")
}

pub fn install(memory: &SharedMemory, regions: &[MemoryRegion]) {
    memory.add_regions(regions).expect("regions install");
}

pub fn run_ready<T>(future: impl Future<Output = T>) -> Option<T> {
    let mut future = Box::pin(future);
    let mut context = Context::from_waker(Waker::noop());
    match Future::poll(Pin::as_mut(&mut future), &mut context) {
        Poll::Ready(value) => Some(value),
        Poll::Pending => None,
    }
}
