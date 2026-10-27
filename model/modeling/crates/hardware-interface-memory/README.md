# hardware-interface-memory

Synchronous byte-memory capabilities, checked regions, aligned allocation, and
local or shared in-memory backing. The only dependency is
`hardware-interface-storage`; no executor, schema, device protocol, or modeled CPU
is required.

`MemoryAccess<S>` carries opaque caller-selected sideband and backend-owned
`Error`. `BoundMemory` fixes that sideband for a capability, and
`SharedMemoryAccess` erases a thread-safe backend without classifying its errors.
The backend selects address spaces and authorization; the library supplies neither
IOMMU translation nor device lifecycle policy. Addresses are byte-addressed `u64`.

Atomic32/64 operations explicitly use naturally aligned **little-endian** words.
They serialize with overlapping accesses in the backend synchronization domain;
unsupported backends return `AtomicAccessError::Unsupported`. No configurable
endianness, bus timing, or cross-device ordering is implied. Ordinary typed helpers
read and write fixed byte arrays without interpreting their encoding.

`Memory<P>` selects a closure-scoped storage policy. `SharedMemory` uses mutex
storage and `LocalMemoryStorage` selects thread-local sharing. Region installation
is all-or-nothing; an allocator advances only after successful installation.

The standalone integration tests are executable non-device examples:
`tests/backend_errors.rs` uses a key-authorized backend with real bytes and a custom
non-Clone error; `tests/memory_sideband.rs` routes identical addresses to independent
banks with opaque sideband. Run `cargo test -p hardware-interface-memory`.

Run `cargo run -p hardware-interface-memory --example bulk_memory` for a standalone
byte-memory probe covering successful transfers, rejected accesses, and allocation
failure without any device or runtime assembly.

## Sideband type guarantees

Even default-constructible metadata cannot be omitted from a tagged backend:

```compile_fail,E0277
use hardware_interface_memory::{MemoryAccess, MemoryAlignment};
#[derive(Default)]
struct Tag;
fn read(memory: &impl MemoryAccess<Tag>) {
    let _ = memory.read(0.into(), MemoryAlignment::ONE, &mut [0]);
}
```

```compile_fail,E0277
use hardware_interface_memory::{MemoryAccess, MemoryAlignment};
#[derive(Default)]
struct Tag;
fn write(memory: &impl MemoryAccess<Tag>) {
    let _ = memory.write(0.into(), MemoryAlignment::ONE, &[0]);
}
```

```compile_fail,E0277
use hardware_interface_memory::{MemoryAccess, MemoryAlignment};
#[derive(Default)]
struct Tag;
fn validate(memory: &impl MemoryAccess<Tag>) {
    let _ = memory.validate(0.into(), MemoryAlignment::ONE, 1);
}
```

```compile_fail,E0277
use hardware_interface_memory::{MemoryAccess, MemoryAlignment};
#[derive(Default)]
struct Tag;
fn read_typed(memory: &impl MemoryAccess<Tag>) {
    let _ = memory.read_typed::<[u8; 1]>(0.into(), MemoryAlignment::ONE);
}
```

```compile_fail,E0277
use hardware_interface_memory::{MemoryAccess, MemoryAlignment};
#[derive(Default)]
struct Tag;
fn write_typed(memory: &impl MemoryAccess<Tag>) {
    let _ = memory.write_typed(0.into(), MemoryAlignment::ONE, &[0]);
}
```

An ordinary backend cannot satisfy a tagged boundary:

```compile_fail,E0277
use hardware_interface_memory::{MemoryAccess, SharedMemory};
struct Tag;
fn tagged_boundary(_: &impl MemoryAccess<Tag>) {}
tagged_boundary(&SharedMemory::new());
```

