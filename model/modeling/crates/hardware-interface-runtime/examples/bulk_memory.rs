//! Manual public-surface probe for checked bulk memory.

use std::cell::Cell;
use std::io::{self, Write};
use std::sync::Arc;

use hardware_interface_runtime::{
    MemoryAccess, MemoryAddress, MemoryAlignment, MemoryRegion, SharedMemory, SharedMemoryAllocator,
};

struct RecordingMemoryAccess<A> {
    inner: A,
    read_calls: Cell<u64>,
    write_calls: Cell<u64>,
}

impl<A> RecordingMemoryAccess<A> {
    const fn new(inner: A) -> Self {
        Self {
            inner,
            read_calls: Cell::new(0),
            write_calls: Cell::new(0),
        }
    }

    fn read_calls(&self) -> u64 {
        self.read_calls.get()
    }

    fn write_calls(&self) -> u64 {
        self.write_calls.get()
    }
}

impl<A: MemoryAccess> MemoryAccess for RecordingMemoryAccess<A> {
    type Error = A::Error;

    fn validate_with_sideband(
        &self,
        (): &(),
        address: MemoryAddress,
        alignment: MemoryAlignment,
        length: u64,
    ) -> Result<(), Self::Error> {
        self.inner.validate(address, alignment, length)
    }

    fn read_with_sideband(
        &self,
        (): &(),
        address: MemoryAddress,
        alignment: MemoryAlignment,
        destination: &mut [u8],
    ) -> Result<(), Self::Error> {
        self.read_calls.set(self.read_calls.get().saturating_add(1));
        self.inner.read(address, alignment, destination)
    }

    fn write_with_sideband(
        &self,
        (): &(),
        address: MemoryAddress,
        alignment: MemoryAlignment,
        source: &[u8],
    ) -> Result<(), Self::Error> {
        self.write_calls
            .set(self.write_calls.get().saturating_add(1));
        self.inner.write(address, alignment, source)
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut surface = io::stdout().lock();
    let happy_memory = SharedMemory::new();
    let alignment = MemoryAlignment::try_new(64)?;
    let region = MemoryRegion::try_new(MemoryAddress::new(0x4000), 64, alignment)?;
    let allocator =
        SharedMemoryAllocator::new(Arc::new(happy_memory.clone()), MemoryAddress::new(0x4000));
    let allocated_address = allocator.allocate(64, alignment)?;

    let mut descriptor = [0x5a; 64];
    if let Some(first) = descriptor.first_mut() {
        *first = 0x11;
    }
    if let Some(last) = descriptor.last_mut() {
        *last = 0xee;
    }
    let happy_access = RecordingMemoryAccess::new(happy_memory.clone());
    happy_access.validate(allocated_address, alignment, 64)?;
    let mut output = [0; 64];
    happy_access.write(MemoryAddress::new(0x4000), alignment, &descriptor)?;
    happy_access.read(MemoryAddress::new(0x4000), alignment, &mut output)?;
    let first = output.first().copied().unwrap_or_default();
    let last = output.last().copied().unwrap_or_default();
    writeln!(
        surface,
        "happy installed_regions={} region_base={:#x} region_length={} region_alignment={} write_calls={} read_calls={} first={first:#04x} last={last:#04x} equal={}",
        happy_memory.regions().len(),
        region.base().raw(),
        region.length(),
        region.alignment().raw(),
        happy_access.write_calls(),
        happy_access.read_calls(),
        output == descriptor
    )?;

    let adverse_memory = SharedMemory::new();
    adverse_memory.add_regions(&[region])?;
    let expected_store = descriptor;
    let setup_access = RecordingMemoryAccess::new(adverse_memory.clone());
    setup_access.write(MemoryAddress::new(0x4000), alignment, &expected_store)?;
    let adverse_access = RecordingMemoryAccess::new(adverse_memory.clone());
    let validation_crossing =
        adverse_access.validate(MemoryAddress::new(0x403f), MemoryAlignment::ONE, 2);
    let mut destination = [0x55, 0x66];
    let before_destination = destination;
    let crossing = adverse_access.write(
        MemoryAddress::new(0x403f),
        MemoryAlignment::try_new(1)?,
        &[0xaa, 0xbb],
    );
    let misaligned = adverse_access.read(
        MemoryAddress::new(0x4001),
        MemoryAlignment::try_new(8)?,
        &mut destination,
    );
    let overflow = adverse_access.read(
        MemoryAddress::new(u64::MAX),
        MemoryAlignment::try_new(1)?,
        &mut [0, 0],
    );
    let verification_access = RecordingMemoryAccess::new(adverse_memory);
    let mut observed_store = [0; 64];
    verification_access.read(MemoryAddress::new(0x4000), alignment, &mut observed_store)?;
    let expected_hash: u64 = expected_store.iter().copied().map(u64::from).sum();
    let observed_hash: u64 = observed_store.iter().copied().map(u64::from).sum();
    writeln!(
        surface,
        "adverse setup_write_calls={} adverse_write_calls={} adverse_read_calls={} verification_read_calls={} crossing={crossing:?} validation_crossing={validation_crossing:?} misaligned={misaligned:?} overflow={overflow:?} destination_before={before_destination:?} destination_after={destination:?} expected_store_hash={expected_hash} observed_store_hash={observed_hash} destination_unchanged={} store_unchanged={}",
        setup_access.write_calls(),
        adverse_access.write_calls(),
        adverse_access.read_calls(),
        verification_access.read_calls(),
        destination == before_destination,
        observed_store == expected_store
    )?;
    let empty_memory = Arc::new(SharedMemory::with_region_capacity(0));
    let limited = SharedMemoryAllocator::new(Arc::clone(&empty_memory), MemoryAddress::new(0x8003));
    let capacity_failure = limited.allocate(64, alignment);
    let first_zero = limited.allocate(0, MemoryAlignment::ONE)?;
    let second_zero = limited.allocate(0, MemoryAlignment::ONE)?;
    writeln!(
        surface,
        "allocator capacity_failure={capacity_failure:?} first_zero={:#x} second_zero={:#x} installed_regions={}",
        first_zero.raw(),
        second_zero.raw(),
        empty_memory.regions().len()
    )?;
    Ok(())
}
