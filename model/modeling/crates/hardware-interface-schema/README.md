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
carry typed IDs, width, alignment, access, reset, and writable-mask metadata. Maps place
registers or nested maps in one declaration-ordered layout.

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

`record_schema!` declares record-level `endian`, optional `alignment`, and ordered
fields whose aligned offsets and final size are generated.
Its field types are scalars, nested records, `enum(storage)`, `bits(storage)` with
LSB0 half-open ranges such as `0..5`, and `union` cases selected by a local or nested
`self` tag. Enum values may omit assignments after an explicit value. Union fields may
use wider wire `storage`, while unused union slots and undeclared bits are reserved
zero. A scalar `{ const: OtherRecord::record_size }` is derived during encoding and
validated during decoding. Generated `*Values` types therefore expose the semantic
record directly rather than requiring a second handwritten overlay.
`bits_schema!` gives one LSB0 half-open bit layout a stable name, typed value, field
identities, inferred minimum scalar extent, and checked codecs. Inferred records refer
to it with `bits(Name)`, while registers use `fields: Name`; both embeddings therefore
share the exact field names, ranges, semantic types, and metadata. Byte order belongs
to the containing record or register rather than to the bits schema.
`register_schema!` declares an offset-free register with explicit width and alignment.
Its inline `fields` use the same bit language: one-bit fields have typed `bool`
accessors and wider ranges use the smallest matching unsigned integer. Field IDs are
inferred in declaration order. Undeclared bits are preserved by field writes, and the
writable mask may not select an undeclared bit. `register_map!`
declares map `alignment`, an optional exact `extent`, and one ordered
`placements: [...]` list. A placement may omit its offset; the macro then aligns the
current high-water mark to the child layout. `count` defaults to one and a repeated
placement's omitted stride is the aligned child extent. Registers and maps implement
the same `RegisterLayout` contract, so maps compose recursively.

Every generated schema boundary has a distinct raw owner and semantic value. `Decode`
converts the raw owner into the semantic value, `Encode` performs the reverse, and the
macros provide matching concrete `TryFrom` implementations. Record and register owners
apply their declared byte order internally, while named bits use an endian-neutral
integer raw owner. A register value defaults to its declared reset value; a register-map
value recursively composes its child defaults.

The following executable example declares a record, two registers, and one ordered
map, then encodes both generated owned-value types.

```rust
use hardware_interface_schema::{bits_schema, record_schema, register_schema};
# use hardware_interface_schema::{
#     Access, BitsFieldDefinition, BitsFieldSpec, BitsLayout, BitsValueError, Endian,
#     Decode, Encode, FieldSpec, RecordLayout, RecordSpec, RegisterCodecError, RegisterDefinition,
#     RegisterLayout, RegisterValueError, ResetKind, ScalarKind, SchemaCodecError,
#     SchemaMemberIdentity, SchemaLocation,
#     inferred_align_up, inferred_field_id, read_scalar, write_scalar,
# };

bits_schema! {
    pub bits CommonFlags(u16) {
        ready: 0..1,
        mode: 8..12,
    }
}

record_schema! {
    pub record FlagRecord {
        endian: Big,
        fields: { flags: bits(CommonFlags), },
    }
}

register_schema! {
    pub register FlagRegister {
        width: 4,
        endian: Little,
        access: ReadWrite,
        reset: Zero(0),
        writable_mask: 0xf01,
        fields: CommonFlags,
    }
}

# fn main() -> Result<(), Box<dyn std::error::Error>> {
let flags = CommonFlagsValues { ready: true, mode: 5 };
assert_eq!(FlagRecord { flags }.to_bytes()?, [0x05, 0x01]);
let mut register = [0xf0, 0xa0, 0x5a, 0xc3];
FlagRegisterViewMut::new(&mut register)?.set_fields(flags)?;
assert_eq!(register, [0xf1, 0xa5, 0x5a, 0xc3]);
# Ok(())
# }
```

```rust
use hardware_interface_schema::{record_schema, register_schema};
# use hardware_interface_schema::{
#     Access, Decode, Encode, Endian, FieldSpec, OrderedRegisterMapSpec, RecordLayout, RecordSpec,
#     RegisterCodecError, RegisterDefinition, RegisterLayout, RegisterLayoutGeometry, RegisterMapPlacement,
#     ResetKind, ScalarKind, SchemaCodecError, SchemaCoordinate, SchemaMemberIdentity,
#     SchemaLocation,
#     checked_align_up, checked_placement_end,
#     read_scalar, write_scalar,
# };
# mod register { pub use hardware_interface_schema::register::*; }
# mod register_map { pub use hardware_interface_schema::register_map::*; }

record_schema! {
    pub record Packet {
        alignment: 4,
        endian: Little,
        fields: {
            sequence: u32,
            code: u16,
        }
    }
}

register_schema! {
    pub register Status {
        width: 4,
        alignment: 4,
        endian: Little,
        access: ReadOnly,
        reset: Zero(0),
        writable_mask: 0,
        fields: {},
    }
}

register_schema! {
    pub register Producer {
        width: 4,
        alignment: 4,
        endian: Little,
        access: ReadWrite,
        reset: Zero(0),
        writable_mask: 0xffff_ffff,
        fields: {},
    }
}

hardware_interface_schema::register_map! {
    pub map DeviceMap {
        alignment: 4,
        extent: 16,
        placements: [
            pub status: Status,
            pub producer: Producer { count: 2 },
        ],
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let packet = Packet { sequence: 0x1122_3344, code: 0x5566 };
    let packet_bytes = packet.to_bytes()?;
    assert_eq!(packet_bytes, [0x44, 0x33, 0x22, 0x11, 0x66, 0x55, 0, 0]);
    assert_eq!(Packet::from_bytes(&packet_bytes)?, packet);
    assert_eq!(Packet::SPEC.validate(), Ok(()));
    let values = DeviceMapValues {
        status: StatusValues::new(7),
        producer: [ProducerValues::new(11), ProducerValues::new(13)],
    };
    let encoded = values.to_bytes()?;
    assert_eq!(&encoded[..12], &[7, 0, 0, 0, 11, 0, 0, 0, 13, 0, 0, 0]);
    assert_eq!(DeviceMap::STATUS.offset(), 0);
    assert_eq!(DeviceMap::PRODUCER.offset(), 4);
    assert_eq!(DeviceMap::PRODUCER.stride(), 4);
    assert_eq!(DeviceMapValues::from_bytes(&encoded)?, values);
    Ok(())
}
```

Generated record value types expose `SPEC`, offset constants, immutable/mutable
checked views, a composable `RecordLayout`, and a generated `*Values` owned codec.
Explicit fixed records additionally expose stable field-ID types. Ordered maps expose
typed placement metadata, checked nested views, and the same owned-codec surface.

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
#     Decode, Encode, Endian, FieldSpec, RecordLayout, RecordSpec, ScalarKind,
#     SchemaCodecError, SchemaMemberIdentity, SchemaLocation, read_scalar, write_scalar,
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
            alignment: 4,
            endian: Little,
            fields: { value: u32, }
        }
    }

    let exporter = RecordCHeaderExporter::new("header.fragment.h", "demo header");
    let mut sink = InMemoryArtifactSink::new();
    let resolved = Header::SPEC.resolve().expect("example schema must resolve");
    assert!(exporter.export(&resolved, &mut sink).is_ok());
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
