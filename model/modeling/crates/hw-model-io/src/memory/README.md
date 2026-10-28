# Byte-memory access and backing

`MemoryAccess` defines byte-memory access independently of backing ownership.
Implement it for your own address space, or choose the supplied `LocalMemory` and
`SharedMemory` models with checked regions and aligned allocation. Both choices
use the same interface; consumers need no allocator or backing-management authority.
All of this module uses only the standard library, without schema or an executor.

The public `storage` module also supports arbitrary shared Rust values:
`LocalStorage<T>` and `MutexStorage<T>` implement closure-scoped `StorageBackend<T>`.
Use them to supply a custom backing policy or share state without memory-address
semantics. Cloning shares the stored value; callback errors do not roll back mutation.
Run `cargo run -p hw-model-io --example callback_result` for those
semantics, and see the module documentation for reentry and lifetime guarantees.

## First byte transfer

Run `cargo run -p hw-model-io --example first_memory`.
Both local and shared backing use the same byte-buffer API; decoding stays explicit:

```rust
use hw_model_io::memory::{
    LocalMemory, MemoryAccess, MemoryAlignment, MemoryRegion, SharedMemory, SharedMemoryAccess,
};
use std::sync::Arc;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let local = LocalMemory::new();
    let shared = SharedMemory::new();
    let region = MemoryRegion::try_new(0.into(), 4, MemoryAlignment::ONE)?;
    local.add_regions(&[region])?;
    shared.add_regions(&[region])?;
    let bytes = 42_u32.to_le_bytes();
    local.write_buffer(0.into(), MemoryAlignment::ONE, &bytes)?;
    shared.write_buffer(0.into(), MemoryAlignment::ONE, &bytes)?;
    // Give a consumer byte-access authority without region-management methods.
    let access = SharedMemoryAccess::from(Arc::new(shared));
    let observed = access.read_buffer::<[u8; 4]>(0.into(), MemoryAlignment::ONE)?;
    assert_eq!(u32::from_le_bytes(observed), 42);
    assert_eq!(local.read_buffer::<[u8; 4]>(0.into(), MemoryAlignment::ONE)?, observed);
    println!("local/shared byte buffers decoded explicitly: 42");
    Ok(())
}
```

`SharedMemoryAccess::same_handle` compares the outer `Arc` identity only.
Cloned capabilities compare equal; independently wrapped `SharedMemory` clones
compare unequal even though they share bytes. It is not an isolation test.

## Capabilities and backing

`MemoryAccess<S>` carries opaque caller-selected sideband and backend-owned
`Error`. `BoundMemory` fixes that sideband for a capability, and
`SharedMemoryAccess` erases a thread-safe backend without classifying its errors.
The backend selects address spaces and authorization; the library supplies neither
IOMMU translation nor device lifecycle policy. Addresses are byte-addressed `u64`.

Atomic32/64 operations explicitly use naturally aligned **little-endian** words.
They serialize with overlapping accesses in the backend synchronization domain;
unsupported backends return `AtomicAccessError::Unsupported`. No configurable
endianness, bus timing, or cross-device ordering is implied. Ordinary byte-buffer helpers
read and write fixed byte arrays without interpreting their encoding.

`Memory<P>` selects a closure-scoped storage policy. `SharedMemory` uses mutex
storage (`SharedMemoryPolicy`); `LocalMemory` uses single-thread clone-sharing
(`LocalMemoryPolicy`). These aliases select `MemoryPolicy` without adding wrappers. Region installation
is all-or-nothing; an allocator advances only after successful installation.

`SharedMemoryAllocator::new(memory, base, size)` checks the exclusive end of a
caller-selected heap and returns `MemoryAllocationError::InvalidRange` on overflow.
Construction installs no backing. Allocation installs only the requested bytes;
alignment padding consumes heap space, and zero-length allocations consume one byte
without installing backing. Empty heaps are valid but exhausted. Clones share the
cursor and limit; requests beyond that limit return `AddressExhausted`. Any failed
allocation leaves the cursor and backing unchanged.

The standalone integration tests are executable non-device examples:
`tests/memory_implementations.rs` implements access over an independent byte array and
feeds it and the supplied backing to the same consumer. `tests/memory_backend_errors.rs`
uses a key-authorized backend with real bytes and a custom
non-Clone error; `tests/memory_sideband.rs` routes identical addresses to independent
banks with opaque sideband. Run `cargo test -p hw-model-io`.

Run `cargo run -p hw-model-io --example bulk_memory` for a standalone
byte-memory probe covering successful transfers, rejected accesses, and allocation
failure without any device or runtime assembly.

## Sideband type guarantees

Even default-constructible metadata cannot be omitted from a tagged backend:

```compile_fail,E0277
use hw_model_io::memory::{MemoryAccess, MemoryAlignment};
#[derive(Default)]
struct Tag;
fn read(memory: &impl MemoryAccess<Tag>) {
    let _ = memory.read(0.into(), MemoryAlignment::ONE, &mut [0]);
}
```

```compile_fail,E0277
use hw_model_io::memory::{MemoryAccess, MemoryAlignment};
#[derive(Default)]
struct Tag;
fn write(memory: &impl MemoryAccess<Tag>) {
    let _ = memory.write(0.into(), MemoryAlignment::ONE, &[0]);
}
```

```compile_fail,E0277
use hw_model_io::memory::{MemoryAccess, MemoryAlignment};
#[derive(Default)]
struct Tag;
fn validate(memory: &impl MemoryAccess<Tag>) {
    let _ = memory.validate(0.into(), MemoryAlignment::ONE, 1);
}
```

```compile_fail,E0277
use hw_model_io::memory::{MemoryAccess, MemoryAlignment};
#[derive(Default)]
struct Tag;
fn read_buffer(memory: &impl MemoryAccess<Tag>) {
    let _ = memory.read_buffer::<[u8; 1]>(0.into(), MemoryAlignment::ONE);
}
```

```compile_fail,E0277
use hw_model_io::memory::{MemoryAccess, MemoryAlignment};
#[derive(Default)]
struct Tag;
fn write_buffer(memory: &impl MemoryAccess<Tag>) {
    let _ = memory.write_buffer(0.into(), MemoryAlignment::ONE, &[0]);
}
```

An ordinary backend cannot satisfy a tagged boundary:

```compile_fail,E0277
use hw_model_io::memory::{MemoryAccess};
use hw_model_io::memory::{SharedMemory};
struct Tag;
fn tagged_boundary(_: &impl MemoryAccess<Tag>) {}
tagged_boundary(&SharedMemory::new());
```


Byte buffers are not decoded Rust values. Scalars require explicit byte decoding:

```compile_fail,E0277
use hw_model_io::memory::{MemoryAccess, MemoryAlignment};
use hw_model_io::memory::{SharedMemory};
let memory = SharedMemory::new();
let _ = memory.read_buffer::<u32>(0.into(), MemoryAlignment::ONE);
```
