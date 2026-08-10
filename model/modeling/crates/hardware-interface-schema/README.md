# hardware-interface-schema

`hardware-interface-schema` is a reusable, runtime-neutral vocabulary for describing
fixed byte records and register maps. It supplies explicit scalar codecs, typed
metadata, checked record views, deterministic validation, exact-start register lookup,
and a format-neutral artifact-export boundary. The facade re-exports the declaration
macros, so downstream crates normally depend on this crate rather than on the
procedural-macro implementation crate.

## What it owns

Schemas are descriptive. A record uses only `u8`, `u16`, `u32`, or `u64` fields with an
explicit `Little` or `Big` byte order. Fields may leave gaps. Register declarations
carry typed IDs, offset, width, access, reset, and writable-mask metadata. Maps combine
fixed registers with repeated banks and return typed locations.

This crate does **not** own ring structure, entry publication, cursors, register
storage, MMIO or bus transactions/effects, DMA, reset epochs, IRQ state machines,
callbacks, actors, concurrency, or an executor. An application consumes its metadata
and decides those runtime concerns.

## Reuse

The workspace marks this crate `publish = false`; this work does not deliver a crates.io
release. A downstream workspace can use a path dependency during local development:

```toml
[dependencies]
his = { package = "hardware-interface-schema", path = "../hardware-interface-schema", default-features = false }
```

It can use the same renamed-dependency form from a Git revision when the repository is
available to that downstream project:

```toml
[dependencies]
his = { package = "hardware-interface-schema", git = "https://example.com/your-org/hardware-interface-schema", rev = "<reviewed-revision>", default-features = false }
```

The macro expansion resolves the facade correctly when it is renamed to `his`. Pin a
revision appropriate to the consuming project; the URL above is a template, not a
publication endpoint.

## Declare and inspect a schema

`record_schema!` declares a fixed record. Scalar fields use
`name: scalar { id, offset, endian }`; arrays use
`name: [scalar; count] { ids: [...], offset, stride, endian }`. `register_schema!`
declares a reusable named set of registers (optionally with an external typed ID as
`Registers<ExternalId>`). `register_map!` declares `size`, inline `fixed` registers,
optional `fixed_groups: { Registers: [Variant, ...] }`, and repeated `banks` with
`count`, `base`, `stride`, and `members`. Register entries always declare `id`, `name`,
`offset`, `width`, `access`, `reset`, and `writable_mask`.

The following executable example declares all three forms, writes bytes through a
generated mutable view, and performs both fixed and repeated exact-start lookup.

```rust
use hardware_interface_schema::{
    record_schema, register_map::RegisterLocation, register_schema,
};
# use hardware_interface_schema::{
#     Endian, FieldSpec, RecordSpec, ScalarKind, SchemaCodecError, read_scalar, write_scalar,
# };
# mod register { pub use hardware_interface_schema::register::*; }
# mod register_map { pub use hardware_interface_schema::register_map::*; }

record_schema! {
    pub record Packet {
        size: 8,
        alignment: 4,
        fields: {
            sequence: u32 { id: 1, offset: 0, endian: Little },
            code: u16 { id: 2, offset: 4, endian: Big },
        }
    }
}

register_schema! {
    pub registers Controls {
        command: { id: 1, name: "command", offset: 0, width: 4, access: ReadWrite,
            reset: Zero(0), writable_mask: 0xffff_ffff },
    }
}

hardware_interface_schema::register_map! {
    pub map DeviceMap {
        size: 64,
        fixed: {
            status: { id: 1, name: "status", offset: 0, width: 4, access: ReadOnly,
                reset: Zero(0), writable_mask: 0 },
        },
        banks: {
            queue: { count: 2, base: 16, stride: 16, members: {
                producer: { id: 2, name: "producer", offset: 0, width: 4, access: ReadWrite,
                    reset: Zero(0), writable_mask: 0xffff_ffff },
            } },
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut bytes = [0_u8; 8];
    let mut view = PacketViewMut::new(&mut bytes)?;
    view.set_sequence(0x1122_3344)?;
    view.set_code(0x5566)?;
    assert_eq!(bytes, [0x44, 0x33, 0x22, 0x11, 0x55, 0x66, 0, 0]);
    assert_eq!(Packet::SPEC.validate(), Ok(()));
    assert!(Controls::SPEC.iter().all(|register| register.validate().is_ok()));
    assert_eq!(DeviceMap::SPEC.validate(), Ok(()));
    assert!(matches!(DeviceMap::locate(0), Some(RegisterLocation::Fixed { .. })));
    assert!(matches!(DeviceMap::locate(32), Some(RegisterLocation::Repeated { instance: 1, .. })));
    assert_eq!(DeviceMap::locate(33), None);
    assert_eq!(DeviceMap::locate(63), None);
    Ok(())
}
```

Generated record marker types expose `SPEC`, field-ID types, offset constants, and
immutable/mutable checked views with typed accessors. Generated register and map marker
types expose `SPEC`, typed `*RegisterId` values, constants, and `register(id)`.

Validation is deterministic. `RecordSpec::validate` checks duplicate IDs, duplicate
names, scalar widths, alignment, extents, and overlaps, in that order and declaration
order within a pass. `RegisterSpec::validate` checks width, writable mask, then reset
value. Map validation checks duplicate IDs/names, duplicate expanded starts, register
metadata, layout/extents, then expanded-range overlap. The macros reject malformed
declarations at compile time; runtime `validate` is available for application-assembled
metadata.

`RegisterMapSpec::locate` is deliberately exact-start-only: it returns `None` for an
interior byte or an unmapped offset. It does not validate transaction width, alignment,
access permission, or reserved ranges, and it has no bus transaction or storage effect.
Those checks belong to the application runtime.

## Export without a storage policy

`Artifact` is one owned `{ logical_name, media_type, contents }` result.
`ArtifactSink` receives artifacts, and `Exporter<S>` renders a schema `S` into a caller
provided sink. They are format-neutral: applications may define their own sinks and
exporters without changing this crate.

```rust
use core::convert::Infallible;
use hardware_interface_schema::{
    export::{Artifact, ArtifactSink, Exporter},
    register_map::RegisterMapSpec,
};

#[derive(Default)]
struct MemorySink(Vec<Artifact>);

impl ArtifactSink for MemorySink {
    type Error = Infallible;

    fn emit(&mut self, artifact: Artifact) -> Result<(), Self::Error> {
        self.0.push(artifact);
        Ok(())
    }
}

struct LayoutExporter;

impl<I> Exporter<RegisterMapSpec<I>> for LayoutExporter
where
    I: Copy + Eq + 'static,
{
    type Error = Infallible;

    fn export<Sink>(
        &self,
        schema: &RegisterMapSpec<I>,
        sink: &mut Sink,
    ) -> Result<(), Self::Error>
    where
        Sink: ArtifactSink<Error = Self::Error>,
    {
        sink.emit(Artifact {
            logical_name: String::from("layout.txt"),
            media_type: String::from("text/plain"),
            contents: format!("map-bytes={}", schema.size()).into_bytes(),
        })
    }
}
```

The sink decides whether artifacts stay in memory, go to a filesystem, or travel to
another application boundary. The crate never writes files for an exporter.

## Optional C11 fragments

The built-in C renderer is additive and default-off. Enable it only when the consumer
needs generic C11 record or register-map fragments:

```toml
[dependencies]
hardware-interface-schema = { path = "../hardware-interface-schema", features = ["export-c-header"] }
```

With that feature enabled, the backend lives at
`export::backends::c_header`; optional backends are not nested outside `export`.
It emits an owned deterministic C fragment through a sink and leaves include guards,
file layout, and integration policy to the caller.

```rust
# use hardware_interface_schema::{
#     Endian, FieldSpec, RecordSpec, ScalarKind, SchemaCodecError, read_scalar, write_scalar,
# };
#[cfg(feature = "export-c-header")]
fn main() {
    use hardware_interface_schema::{
        export::{Exporter, InMemoryArtifactSink},
        export::backends::c_header::RecordCHeaderExporter,
        record_schema,
    };

    record_schema! {
        record Header {
            size: 4,
            alignment: 4,
            fields: { value: u32 { id: 1, offset: 0, endian: Little }, }
        }
    }

    let exporter = RecordCHeaderExporter::new("header.fragment.h", "demo header");
    let mut sink = InMemoryArtifactSink::new();
    assert!(exporter.export(&Header::SPEC, &mut sink).is_ok());
    let artifacts = sink.into_artifacts();
    assert_eq!(artifacts.len(), 1);
    assert_eq!(artifacts[0].logical_name, "header.fragment.h");
    assert!(artifacts[0].contents.starts_with(b"#define C_DEMOQX20HEADER_SIZE"));
}

#[cfg(not(feature = "export-c-header"))]
fn main() {}
```

No Markdown, JSON, or PDF exporter is implemented. This crate does not provide `no_std`,
a callable C API, native-layout ABI casts, PCIe/MSI-X behavior, SR-IOV, FLR, or runtime
effects.

## PDMA compatibility

PDMA is a compatibility consumer of this facility. Its four record schemas and the
exact 66-register, 4 KiB little-endian map retain the same bytes, public APIs, error
precedence, generated C header, and runtime ownership. Execution, retirement, reset,
and IRQ behavior remain in PDMA's contract, device, host, and scenario crates. This
extraction is reusable infrastructure, not new PDMA hardware behavior.
