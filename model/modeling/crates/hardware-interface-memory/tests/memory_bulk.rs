#![allow(missing_docs, reason = "integration test crate")]
#![allow(clippy::expect_used, reason = "checked test fixture operations")]

mod support;

use hardware_interface_memory::{MemoryAccess, MemoryAddress, SharedMemory};
use support::{alignment, install, region};

#[test]
fn memory_bulk_descriptor_round_trips_in_one_write_and_one_read() {
    // Given one region and a literal 64-byte descriptor.
    let memory = SharedMemory::new();
    let installed = region(0x4000, 64, 64);
    install(&memory, &[installed]);
    let mut descriptor = [0x5a; 64];
    if let Some(first) = descriptor.first_mut() {
        *first = 0x11;
    }
    if let Some(last) = descriptor.last_mut() {
        *last = 0xee;
    }
    let mut output = [0; 64];

    // When one bulk write and one bulk read are issued.
    memory
        .write(MemoryAddress::new(0x4000), alignment(64), &descriptor)
        .expect("bulk write");
    memory
        .read(MemoryAddress::new(0x4000), alignment(64), &mut output)
        .expect("bulk read");

    // Then metadata and every literal byte survive the public surface.
    assert_eq!(
        memory.regions().expect("metadata is accessible"),
        vec![installed]
    );
    assert_eq!(output, descriptor);
    assert_eq!(output.first(), Some(&0x11));
    assert_eq!(output.last(), Some(&0xee));
}
