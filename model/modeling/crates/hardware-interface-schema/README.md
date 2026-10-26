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

`RegisterWindow<M>` owns one generated register value at a caller-selected base.
Its exact-leaf MMIO access checks window bounds, width, permissions and writable
bits. Synchronous per-access hooks let hardware validate a proposed write, refresh
a read or apply effects after acceptance; hooks are never stored. Failed validation
or a rejecting hook preserves the window values and read destination. Effects on
external captures cannot be rolled back.

`register_dispatch!` evaluates access expressions lazily and falls through only on
`RegisterAccessError::OutsideWindow`. Gaps inside a window and rejected accesses are
terminal. Dispatch requires non-overlapping windows, as established by generated
placements. Applications own ring structure, entry publication, cursors, DMA, reset
epochs and IRQ behavior. Storage backends, bus observation, actors, concurrency and
execution remain outside this crate.

This crate exposes no ring protocol or ring types and has no normal dependency on
PDMA crates. An entry type parameter does not make application cursor rules schema
mechanics; PDMA defines those rules in its contract crate.

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

### Descriptions

Inside declaration macros, `#[short_description = "..."]` and
`#[description = "..."]` attach optional text to schema definitions and members:

```rust
hardware_interface_schema::record_schema! {
    pub record CopyRequest {
        endian: Little,
        fields: {
            #[short_description = "Transfer length"]
            #[description = "Number of bytes to copy.\nZero performs no transfer."]
            length: u32,
        },
    }
}
```

The same attributes describe register fields, reusable bits, and register-map
placements. The outer schema macro consumes these attributes; they are not separate
attribute macros. Both accept string literals, including raw and multiline strings.
Duplicate attributes and non-string values are errors.

Descriptions live in existing schema metadata as `Documentation`. A placement may
override its reused definition's text; each omitted item falls back independently.
An explicit empty string overrides rather than inherits. Parent map descriptions do
not become descriptions of child registers. Descriptions never change layout,
encoded bytes, access permissions, or reset values.

Generated Rust documentation and the optional C exporter use these descriptions.
Metadata preserves the source text; the C renderer neutralizes comment delimiters,
trigraphs and line splicing when writing comments. An undocumented declaration keeps
its existing C output. Other documentation tools can read the same metadata without
parsing Rust source or generated comments.

Run the complete example to generate a described record and register header:

```bash
cargo run -p hardware-interface-schema --features export-c-header --example documented_schema
```

### Layout and values

`record_schema!` declares record-level `endian`, optional `alignment`, and ordered
fields whose aligned offsets and final size are generated.
Its field types are scalars, nested records, `enum(storage)`, `bits(storage)` with
LSB0 half-open ranges such as `0..5` or inclusive ranges such as `0..=4`, and `tagged_union`
cases selected by a local or nested `self` tag. Enum values may omit assignments after
an explicit value. Union fields may
use wider wire `storage`, while unused union slots and undeclared bits are reserved
zero. A scalar `{ const: OtherRecord::record_size }` is derived during encoding and
validated during decoding. Generated `*Values` types therefore expose the semantic
record directly rather than requiring a second handwritten overlay.
`bits_schema!` gives one LSB0 bit layout a stable name, typed value, field
identities, inferred minimum scalar extent, and checked codecs. Inferred records refer
to it with `bits(Name)`, while registers use `fields: Name`; both embeddings therefore
share the exact field names, ranges, semantic types, and metadata. Byte order belongs
to the containing record or register rather than to the bits schema.
`register_schema!` declares an offset-free register with explicit width and alignment.
Its inline `fields` use the same bit language: one-bit fields have typed `bool`
accessors and wider ranges use the smallest matching unsigned integer. `0..=0` selects
only bit 0, `7..=9` selects three bits, and `0..=63` selects an entire 64-bit word.
The existing `..` spelling stays half-open; both forms can be mixed in a declaration.
Inclusive endpoints are normalized at parsing, so codecs, metadata and overlap checks
use the same internal layout. Descending, overlapping and out-of-storage ranges are
rejected, as are inclusive endpoints whose conversion would overflow.
Field IDs are inferred in declaration order. Undeclared bits are preserved by field writes, and the
writable mask may not select an undeclared bit. `register_map!` declares an optional
exact `extent`, ordered `placements: [...]`, and optional `access: [...]` tree. Access
paths name existing placements and generate borrowed views of the root bytes. A
placement may omit its offset; the macro aligns the high-water mark. A placement without
brackets is scalar; `[count]` declares an indexed bank, including `[1]`. Bank strides
always use the aligned child extent. Maps and registers share
one composable `RegisterLayout`; access namespaces define no layout, IDs, or storage.

Scalar record enums and enum bit fields share declaration parsing, discriminant
validation, typed conversions and enum metadata. A bit field adds its range before
the enum declaration:

```rust
use hardware_interface_schema::bits_schema;

bits_schema! {
    pub bits Status(u8) {
        code: 0..2 => enum {
            variants: { Idle = 0, Active = 1, Fault = 2 },
        },
    }
}
```

The same field syntax works inside register fields and record `bits(storage)`.
Named bits layouts can be embedded in both containers. Enum variants may carry
`description` and `short_description`. Values must fit the declared bit range;
duplicate names/codes and overflowing implicit values are rejected during expansion.
Unknown raw codes produce a located `UnknownTag` error when decoded as an enum.
Register raw storage still preserves those codes and undeclared bits. An owned
enum field setter is infallible because every variant fits by construction; typed
reads remain fallible. Reading named register `fields()` is now fallible as well.
The C exporter emits unshifted discriminant constants and variant documentation
from the shared metadata, including nested and repeated placements.

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
            pub producer: Producer[2],
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
exporters without changing this crate. `ExportError<RenderError, SinkError>` keeps rendering
and storage failures distinct and preserves each typed cause. `Exporter::Error` describes
rendering only; sinks choose their own error type. `InMemoryArtifactSink` is infallible.

```rust
use core::convert::Infallible;
use hardware_interface_schema::{
    export::{Artifact, ArtifactSink, ExportError, Exporter},
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
    ) -> Result<(), ExportError<Self::Error, Sink::Error>>
    where
        Sink: ArtifactSink,
    {
        sink.emit(Artifact {
            logical_name: String::from("layout.txt"),
            media_type: String::from("text/plain"),
            contents: format!("map-bytes={}", schema.size()).into_bytes(),
        }).map_err(ExportError::Sink)
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
a callable C API, native-layout ABI casts, PCIe/MSI-X behavior, SR-IOV, FLR, or
hardware-specific effects.

## Constructor-bound register banks

Map placements use brackets for repetition: `pub channels: Channel[4]` or
`pub channels: Channel[channel_count] { offset: 16 }`.
Brackets preserve indexed identity even for `[1]`; the only placement option is
`offset`. Counts are integer literals or declared `usize` parameters. Stride is
always `align_up(child.extent(), child.alignment())`; declare reserved padding in
the child block's extent. Neither the DSL nor `RegisterBank::new(child, base, count)`
accepts a stride override.

Declare parameters with `pub map Device(channel_count: usize) { ... }`.
`Device::new(channel_count)` returns a checked, immutable `DeviceLayout`, containing
geometry rather than aggregate hardware values. Its `extent()` and typed placement
handles use the constructor-bound count. A child can receive parameters recursively:
`pub groups: Group(lanes)[groups]`. Parameterized child placements always produce an
instance layout, including literal arguments such as `Group(3)[1]`.

Each handle exposes `base()`, `count()`, `stride()`, checked `offset(index)`,
`locate(address)`, and `route(address, |index, child_offset| ...)`. Route closures
borrow caller-owned hardware and use the existing `RegisterAccessError` and
`register_dispatch!` protocol. A block reserves its complete address envelope,
including internal holes. Sibling placements cannot overlap a child or bank envelope;
group registers sharing one repeated slot inside a single child block. `locate` and
`route` select a child aperture so its own validation handles reserved holes.
`locate_instance` selects a configured
stride slot, including its padding, before the caller checks permissions and aperture
bounds; `finish_access` reports the terminal hole or
outside-window error after sibling dispatch. `rebased(parent_base)` translates a
bank into a parent address space without allocating hardware or metadata.

Static prefixes retain inferred `MEMBER_OFFSET` and `MEMBER_STRIDE` constants.
Fixed maps retain their `Copy` values, const extent, views and `RegisterWindow` API.
Explicit `resolve()` collects owned bank templates, counts, strides, documentation
and leaf ancestry. Access projection retains bank geometry when members are hidden.
C exports preserve flattened leaf symbols and include bank geometry plus parent
bank/instance links for nested banks.

Low-level `RepeatedRegisterBank` metadata retains descriptive strides. Both
`RegisterMapSpec::validate()` and `resolve()` reject overlap across complete bank
envelopes, including a scalar or another bank placed in an internal hole.

## PDMA consumer

PDMA uses generated layouts for an instance-configured channel count, independent
command/error-event rings and DMA/channel controls. Its current contract generates
the C header; historical register counts and layouts are not compatibility targets.
PDMA ring geometry and pointer rules belong to its contract crate; shared software
ring algorithms belong to `pdma-model-software`. DMA, reset and IRQ behavior belong
to the PDMA hardware and software, not this schema library.

The resulting layer boundary is deliberate: this crate owns schema descriptors,
codecs, checked views and generated-value register windows.
`hardware-interface-runtime` supplies storage backends and bus/observation algorithms,
`hardware-interface-pipeline`
supplies interaction drive policies, and the PDMA device supplies register command,
DMA, reset and IRQ effects.

### Record-backed union cases

An inline `tagged_union` requires a `tag: self...` discriminator. The old inline
`union` spelling is rejected with a migration hint; replace that field keyword with
`tagged_union`. This syntax change does not alter generated Rust names or wire layout.

In addition to equal-width inline cases, a record union can reuse complete schemas:
```rust
# use hardware_interface_schema::{record_schema, SchemaCodecError};
# fn main() -> Result<(), SchemaCodecError> {
record_schema! { pub record Small { endian: Little, fields: { value: u16 } } }
record_schema! { pub record Large { endian: Little, fields: { first: u64, second: u64 } } }
record_schema! {
    pub record Packet {
        endian: Little,
        fields: {
            tag: enum(u8) { variants: { Small = 0, Large = 1 } },
            payload: tagged_union { tag: self.tag, cases: { Small(Small), Large(Large) } },
        },
    }
}

// Given: differently sized records selected by the union value.
let small = Packet::new(
    PacketTag::Large,
    PacketPayloadValues::Small(Small { value: 0x1234 }),
);
let large = Packet::new(
    PacketTag::Small,
    PacketPayloadValues::Large(Large { first: 7, second: 42 }),
);
// When: both variants are encoded.
let small_bytes = small.to_bytes()?;
let large_bytes = large.to_bytes()?;
// Then: the union determines each tag, and both records round trip.
assert_eq!(small_bytes.first(), Some(&0));
assert_eq!(large_bytes.first(), Some(&1));
assert_eq!(Packet::from_bytes(&small_bytes)?, small);
assert_eq!(Packet::from_bytes(&large_bytes)?, large);
# Ok(())
# }
```
All branches in that union use record syntax. Size and alignment are the maximum
of the selected record definitions, rounded for alignment. Encoding derives the
tag from the value; decoding rejects nonzero storage beyond the selected record.
Nested codec errors retain their rebased coordinates. The parent specification
marks an opaque union region; each branch retains its own record specification.
The C exporter emits nested-record OFFSET/SIZE constants, so separately exported
branch accessors can operate on that region without handwritten layout offsets.
With multiple inline unions in one record, raw word accessors and metadata are
prefixed by the union field name; a single inline union retains word0/word1 names.

### Packed enum union tags

A union can select an enum member of an inline `bits(storage)` field or a reusable
`bits(Name)` field with `tag: self.flags.kind`. The member must be an enum; an
ordinary integer or boolean bitfield cannot serve as a discriminator. Cases must
cover exactly the enum variants. Inline declarations are checked by the macro;
referenced enum types are checked by the generated exhaustive Rust matches.

```rust
# use hardware_interface_schema::{bits_schema, record_schema, SchemaCodecError};
# fn main() -> Result<(), SchemaCodecError> {
bits_schema! {
    pub bits Header(u16) {
        ready: 0..=0,
        kind: 4..=5 => enum { variants: { Small = 1, Large = 2 } },
    }
}
record_schema! {
    pub record Packet {
        endian: Little,
        fields: {
            header: bits(Header),
            payload: tagged_union { tag: self.header.kind,
                cases: { Small { value: u32 }, Large { first: u32, second: u32 } },
            },
        },
    }
}
let packet = Packet::new(
    HeaderValues { ready: true, kind: HeaderKind::Small },
    PacketPayloadValues::Large(PacketPayloadLargeValues { first: 7, second: 42 }),
);
let bytes = packet.to_bytes()?;
assert_eq!(packet.header.kind, HeaderKind::Large);
assert!(packet.header.ready);
assert_eq!(Packet::from_bytes(&bytes)?, packet);
# Ok(())
# }
```

Owned constructors and encoders derive the discriminator from the payload while
preserving other semantic fields in the same word. Separate unions can use
different enum members of that word. Existing checks for unknown enum values,
reserved bits, inactive union storage and sibling-field overflow still apply.
The containing record supplies the word's byte order; bit ranges remain LSB0.

Tag lookup remains local: `self.kind`, `self.header.kind` for a nested record enum,
or `self.flags.kind` for a bits member. It does not traverse to a parent record or
scatter a child record's fields. The C exporter retains its unshifted enum constants
and raw-word load/store interface; C callers pack the control word according to
the declared bit positions and select consistent payload bytes.

### Untagged complete-record unions

`union_schema!` overlays independently defined complete records at byte zero:

Its `union` declaration is untagged and cannot contain a `tag` clause. The caller
selects the case explicitly; use an inline `tagged_union` in a record when the wire
format carries a discriminator.

```rust
use hardware_interface_schema::{record_schema, union_schema};
record_schema! { pub record Small { endian: Big, fields: { value: u16 } } }
record_schema! { pub record Large { endian: Little, fields: { value: u64 } } }
union_schema! { pub union Packet { cases: { Small(Small), Large(Large) } } }
let value = Packet::Small(Small::new(0x1234));
let mut raw = PacketRaw::default();
value.encode_variant(raw.as_mut())?;
assert_eq!(Packet::decode_variant(raw.as_ref(), PacketCase::Small)?, value);
# Ok::<(), hardware_interface_schema::SchemaCodecError>(())
```

The explicit decoder takes `PacketCase`; the macro never infers or reads a selector.
`RECORD_SIZE` is the largest case extent rounded to the largest case `RECORD_ALIGNMENT`.
`ZERO` selects the first record's semantic zero. `PacketRaw` owns unvalidated bytes
and provides `new`, `as_bytes`, `into_bytes`, `Default`, `AsRef` and `AsMut`.
`PacketView` and `PacketViewMut` check extent at construction and expose
`variant(case)`; the mutable view also exposes atomic `set_variant(&value)`.
Decoding checks the selected record, its direct alignment gaps and trailing
padding, and the union's inactive tail. Nested records keep their own padding
policy; ordinary record decoding remains permissive about alignment padding.
Encoding uses temporary fixed-size storage, zeros inactive bytes, and commits
only after the selected record encodes successfully.

By default the union has no `Decode`, `Encode`, `RecordLayout`, `to_bytes`, or
`from_bytes` and needs no application codec hooks. `Packet::SPEC` describes only
opaque storage extent and alignment. Export each case's ordinary `SPEC` alongside
it when consumers need field accessors; no selector metadata is generated.

An optional `codec: external` clause generates mechanical adapters for an
application-provided codec:

```rust
use hardware_interface_schema::{Decode, Encode, SchemaCodecError, record_schema, union_schema};
record_schema! { pub record Small { endian: Big, fields: { value: u16 } } }
record_schema! { pub record Large { endian: Little, fields: { value: u64 } } }
union_schema! {
    pub union Packet {
        cases: { Small(Small), Large(Large) },
        codec: external,
    }
}
impl Packet {
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, SchemaCodecError> {
        // This application always interprets incoming storage as Small.
        Self::decode_variant(bytes, PacketCase::Small)
    }
    pub fn encode_into(&self, bytes: &mut [u8]) -> Result<(), SchemaCodecError> {
        self.encode_variant(bytes)
    }
}
let value = Packet::Small(Small::new(0x1234));
assert_eq!(value.encode()?.decode()?, value);
assert_eq!(PacketView::new(&value.to_bytes()?)?.values()?, value);
# Ok::<(), SchemaCodecError>(())
```

The opt-in adds value `Encode`, raw `Decode`, fallible conversions in both
directions, `RecordLayout`, value `to_bytes`, immutable-view `Decode`, both views'
`values()`, and mutable-view `set_values(&value)`. All codec paths delegate to
the application's inherent `from_bytes(&[u8]) -> Result<Self, SchemaCodecError>`
and `encode_into(&self, &mut [u8]) -> Result<(), SchemaCodecError>`. Missing hooks
fail compilation. Hook implementations can live in another module with ordinary
Rust visibility; the views and adapters are generated at the union declaration.

The schema adds no selector requirement, inference, protocol validation, or
callback trait. Application hooks own case selection, validation, errors, and
write atomicity; adapters preserve their behavior. View construction checks only
extent and clips the borrow to the union size. `variant(case)` and
`set_variant(&value)` remain explicit structural operations even with the opt-in,
so they do not apply application codec policy. The example's fixed-case decoder
needs no selector at all. Inline `tagged_union` behavior is unchanged.
