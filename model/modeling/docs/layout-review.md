# Register and descriptor layout review

The current command slot is 96 bytes, naturally aligned to eight, with one shared
header/opcode definition and complete records overlaid by an untagged union.
The [2026-10-26 revision](#2026-10-26-revision-shared-headers-and-untagged-command-storage)
records why the first complete-record design was discarded and how selection is
kept outside generic union storage. The current wire table is in
[copy-engine.md](copy-engine.md#wire-layout).

The context-header, channel-group and control-configuration changes below are
implemented. The following inventory and initial command decision are retained
as the **2026-10-23 baseline**, measured at commit
`dfcc3ff2a0340f8898bbee4f6706a83959247ca2`. They explain the earlier 112-byte format,
not the current ABI. Offsets are decimal bytes relative to the containing
record/map. Wire records are little-endian; Rust object sizes are not wire sizes.

## Initial record inventory (historical)

Sources: [commands](../crates/contract/src/dma/command.rs),
[endpoints](../crates/contract/src/dma/endpoint.rs),
[operations](../crates/contract/src/dma/operations.rs),
[atomics](../crates/contract/src/dma/operations/atomic.rs),
[events](../crates/contract/src/dma/event.rs),
[management records](../crates/contract/src/control.rs) and
[context attributes](../crates/contract/src/memory.rs).

| Record | Bytes | Alignment | Layout or packing observation |
| --- | ---: | ---: | --- |
| Command | 112 | 8 | `id@0`, `kind@8`, 4-byte gap, 96-byte payload union at 16 |
| CopyCommand | 96 | 8 | Source endpoint at 0, destination at 40, length at 80, SG context at 88, 4-byte tail |
| Endpoint | 40 | 8 | Mode at 0, context at 4, 32-byte union at 8 |
| Position | 24 | 8 | Mode at 0, 4-byte gap, 16-byte union at 8; not an additional Command field |
| Segment | 16 | 8 | Address and length, both 64 bits; no padding |
| WordTarget | 16 | 8 | Address at 0, context at 8, width at 12; no padding |
| WordWrite | 24 | 8 | WordTarget followed by 64-bit value; shared by Write and Fence |
| FillCommand | 32 | 8 | Address, length, pattern, context and width; no padding |
| WaitCommand | 32 | 8 | WordTarget, value and comparison; 4-byte tail |
| AtomicCommand | 40 | 8 | WordTarget, order, operation and 16-byte operand union |
| SemaphoreCommand | 24 | 8 | WordTarget and operation; 4-byte tail |
| TimestampCommand | 16 | 8 | Address and context; 4-byte tail |
| TrapCommand | 0 | 8 | No payload; still occupies a full Command ring slot |
| Event | 24 | 8 | ID at 0, code at 8, 4-byte gap, 8-byte info union at 16 |
| ErrorInfo | 8 | 8 | Committed byte count |
| ContextAttributes | 8 | 4 | Address-space mode and PASID, both stored as 32 bits |
| ControlRequest | 32 | 8 | ID, opcode, Function, channel, context and attributes; no padding |
| ControlResponse | 24 | 8 | ID, status, channel and context; 4-byte tail |
| ControlRingState | 24 | 8 | Base, capacity, producer and consumer; 4-byte tail |
| ControlQueue | 72 | 8 | Three ControlRingState records: command, response and event |
| ControlEvent | 8 | 4 | Event kind and code; no padding |

An Endpoint's Linear case uses 16 of its 32 payload bytes; its SG case uses all
32: table address, count, starting index and local offset, each 64 bits. A Copy
therefore retains two independent contexts and positions plus one SG-table
context. The union's inactive storage is distinct from alignment padding.

The fixed Command slot reserves 96 payload bytes for every operation. Write and
Fence use a 24-byte payload, Timestamp uses 16, and Trap uses none. Reducing one
of those payloads does not reduce the ring stride while Copy remains the largest.

## Command encoding decision

Keep fixed-size slots for this pass. Variable-length packets, indirect commands
and extension descriptors introduce fetch and lifetime rules beyond layout
cleanup. Keep full-width IDs, addresses, lengths, SG counts/indices/offsets and
context selectors unless a separately documented limit justifies narrowing them.

With these fields retained, a double-SG Copy has 92 bytes of scalar information
before opcode/mode tags: 8-byte ID, three 4-byte contexts, 8-byte length and two
32-byte SG descriptions. A self-contained 64-byte descriptor cannot preserve
those independent values.

| Candidate | Slot bytes | Change from current | Consequence |
| --- | ---: | ---: | --- |
| Keep current nested Endpoint records | 112 | — | Simple reusable endpoint encoding; small operations retain large unused tails |
| Reorganize command header and Copy metadata | 96 | −14.3% | Feasible byte budget, but requires a new union/header organization |
| Round every slot to a power of two | 128 | +14.3% | Regular address arithmetic; more bytes fetched/stored, without a demonstrated platform need |

A concrete **96-byte packing sketch**, preserving the numeric field widths, is:

| Offset | Bytes | Copy field |
| ---: | ---: | --- |
| 0 | 4 | Opcode and source/destination mode bits, with reserved bits |
| 4 | 4 | SG-table context |
| 8 | 8 | Command ID |
| 16 | 8 | Copy length |
| 24 | 4 | Source context |
| 28 | 4 | Destination context |
| 32 | 32 | Source Linear/SG parameters |
| 64 | 32 | Destination Linear/SG parameters |

This sketch is not a schema-validated replacement. It shares header space with
operation-specific metadata and separates an endpoint's mode/context from its
parameter union. Other operations would need defined layouts and reserved-byte
rules in that same envelope. Before choosing it, verify that schema can express
the discriminants, unions and generated C accessors coherently; do not add a
handwritten PDMA codec beside schema or leak PDMA concepts into generic macros.

**2026-10-23 implementation decision: retain 112 bytes.** At that checkpoint the
[union validator](../crates/hw-model-schema-macros/src/internal/validate/record/modern.rs)
accepted a local enum or one nested-record enum as a tag, not a bitfield tag or a
reference into the parent record. The
[layout generator](../crates/hw-model-schema-macros/src/internal/expand/record/modern/layout.rs)
placed whole nested records; it could not scatter Endpoint fields into Command's
header. Byte-sized tags would address the bitfield restriction but not the parent
scope or record-placement restrictions. Reaching 96 bytes would require a broader
schema/type redesign, or a separate wire representation and conversion layer.
Neither is justified by this layout cleanup. Keep the existing cohesive typed
records and generated codecs; do not impose new SG or context limits.

The 2026-10-26 follow-up adds enum-bitfield union tags, documented in
[Packed enum union tags](../crates/hw-model-schema/README.md#packed-enum-union-tags).
Contiguous child records remain intentional. A body's discriminant header should
belong to that body, keeping its decoding self-contained; parent-tag lookup and
scattered child fields are not prerequisites for revisiting the command design.
The schema enhancement itself leaves the PDMA descriptor encoding unchanged.

At this checkpoint the 96-byte sketch remained a possible future encoding, not an outstanding task in
this implementation. No measured bus-throughput improvement follows from its byte
count alone. In particular, 96-byte stride does not imply 64-byte slot alignment.

Keep Event at 24 bytes for now: ID and progress already require 16 bytes before
the error tag. Moving the gap to the tail saves no space; rounding to 32 adds
one third. Keep the 32-byte management request and naturally aligned 24-byte
response/ring state. Opcode-unused fields are not automatically unnecessary:
their common envelope keeps decoding and software ownership straightforward.

## DMA register apertures

Sources: [DMA maps](../crates/contract/src/dma/registers.rs) and
[channel maps](../crates/contract/src/dma/registers/channel.rs).

For `N` channels, discovery occupies bytes 0–7. Administration begins at 8,
contains 12 bytes of global controls followed by `N` four-byte channel-reset
blocks, and ends at `20 + 4*N`. Channel data begins at
`align_up(20 + 4*N, 4096)`; each complete ChannelBlock occupies 4096 bytes.
For three channels the complete aperture is 16384 bytes.

Keep this page separation. Channel reset belongs in privileged administration;
Resume belongs in the owner's work window. They must not become neighbors merely
because both affect recovery. Discovery may share a page with administration:
mapping that page wholesale to an untrusted requester is not a substitute for
the managed access filter. Root base addresses remain platform-selected.

Each channel has 76 bytes of registers within an 80-byte prefix:

| Group | Baseline offsets | Implemented offsets |
| --- | --- | --- |
| Command ring | Base 0 (8 bytes), capacity 8, producer 12, consumer 16 | Unchanged |
| Alignment gap | 20–23 | Unchanged |
| Event ring | Base 24 (8 bytes), capacity 32, producer 36, consumer 40 | Unchanged |
| Configuration | Enable 44, ring context 64 | Enable 44, ring context 48 |
| Failure and resume | Fatal 52, fault status 68, resume 72 | Fatal 52, fault status 56, resume 60 |
| Interrupts | Enable 48, cause 56, mask 60, Trap ACK 76 | Enable 64, cause 68, mask 72, Trap ACK 76 |
| Reserved aperture | 80–4095 | Unchanged |

The groups are nested schema maps with four-byte alignment:
`ChannelRegisters.config: ChannelConfiguration`,
`recovery: ChannelRecovery` and `irq: ChannelInterrupts`. They remain owned by
the channel and need no new controller or duplicated storage.
Ring producer/consumer words remain separate: each has a different writer.
RO status, ordinary RW configuration and write-one actions also remain separate;
packing them together would complicate update/acknowledgment behavior.

## Managed registers

Sources: [contexts](../crates/contract/src/hw_components/context.rs),
[control channel](../crates/contract/src/hw_components/control.rs),
[windows](../crates/contract/src/hw_components/windows.rs) and
[firmware boot](../crates/contract/src/hw_components/firmware.rs).

| Map | Baseline extent / alignment | Implemented decision |
| --- | --- | --- |
| ContextManagementRegisters | 20 / 8 | Capacity at 0, result at 4, request at 8; extent is now 16 |
| ContextEntryRegisters | 16 / 4 | Owner at 0, attributes at 4, valid at 12; retain whole-entry bank structure |
| ContextAttributeRegisters | 8 / 4 | Mode at 0, PASID at 4; leave extensibility room rather than narrowing the whole entry |
| ChannelFailures | 4 / 4 per word | Keep bitmap bank last, with `ceil(N/32)` words |
| ControlRegisters | 36 / 8 | Config holds bootstrap at 0, attributes at 8, enable at 16; request at 20, notifications at 24 |
| ControlNotificationRegisters | 12 / 4 | Response doorbell, event doorbell, event error; already cohesive |
| ChannelWindowBlock | 12 / 4 | Open, owner, ready; whole-block bank stride is 12 |
| FirmwareRegisters | 4 / 4 | One atomic boot-state/error word; retain independent lifetime |

The context-management change removes the gap before the atomic 64-bit request.
For `K` entries, entries begin at 16 rather than 20, and the failed bitmap
begins at `16 + 16*K` rather than `20 + 16*K`. This saves four bytes per
table and makes entry offsets multiples of 16. It does not require a 16-byte
transaction or silently increase the root's existing eight-byte alignment.
Keep context ID and action together in the request; keep the failed bitmap
separate and last. Firmware still publishes/clears valid.

For control bootstrap, use `bootstrap@0, attributes@8, enable@16`
in `ControlRegisters.config: ControlConfiguration`. Request and notification offsets remain unchanged.
This saves no bytes, but places the publication enable after the state it
publishes. Retain distinct response/event doorbells and event error status.
PF control-memory attributes remain independent of the DMA context table.

Keep the permission-bank stride at 12 unless an RTL decode requirement calls for
16. A 16-byte stride would add four reserved bytes per channel; it is not needed
for the current four-byte registers. Do not interleave owners, enables or statuses
from different channels. Static Host/Firmware header views do not replace runtime
PF authorization or per-Function window ownership checks.

## Bit widths and compatibility rules

Small enum encodings deserve review, but the benefit must be measured at the
containing word/record. Narrowing Endpoint.mode alone still leaves alignment or
context placement consuming its current header space. Likewise a one-bit switch
in a 32-bit MMIO word uses a deliberate access granularity; it does not by itself
justify byte-sized MMIO accesses.

PASID uses 20 meaningful bits; Default and PASID zero must remain distinct.
Context selectors and Function IDs must not be truncated to match today's small
test instances. Ring counters retain 32-bit wrapping semantics and their
capacity constraint. A narrower fatal-code field could reserve bits for growth,
but would not save its 32-bit register and must preserve rejection rules.

Any approved layout change must preserve explicit byte order, reserved-field
validation, access widths, masks, reset domains and publication boundaries.
Descriptions must distinguish reserved bytes, inactive union bytes and permitted
future extensions. Extendable context attributes are not permission to reinterpret
existing live bindings.

## Validation and implementation boundary

The inventory resolves 21 record specifications, five root maps at three channels
and three contexts, and instance shapes at 1, 3, 32, 33, 1019 and 1020 channels.
The last two exercise the administration area's crossing of a page boundary;
32/33 exercise failed-bitmap growth. Channel data starts at 4096 for 1019 channels
and at 8192 for 1020. Generated C headers are checked against the current schema.
Raw local evidence is under `.omo/evidence/layout-audit/`; this document retains
the sizes, placement rationale and decisions needed without that temporary data.

The implementation changes the context header and channel/control groups without
descriptor repacking. Host and firmware headers are regenerated from schema; the
common memory-record header remains unchanged. Consumers use generated nested
offsets rather than compatibility aliases or handwritten numeric routing.
Independent ABI fixtures cover the new placements and permissions; existing
wire fixtures, malformed encodings, bank boundaries and direct/managed scenarios
cover the surrounding behavior. New layout tests pin the external contract,
not arithmetic copied from the implementation.

The descriptor and memory-record layouts remain unchanged. Register consumers and
generated headers must use the regrouped maps together; old offsets and flattened
field paths are not compatibility interfaces.

Final validation passed the default workspace suite (1130 tests), all-features
suite (1174 tests), formatting, strict clippy, private-item documentation and
generated-header consistency checks. Neither suite has ignored tests. Review also
removed a legacy numeric enable address from the firmware idle-test bus while
preserving its exactly-one-read assertion.

## 2026-10-26 revision: shared headers and untagged command storage

The first uncommitted complete-record tagged-union implementation was discarded
at the user's request, without a backup branch. The retained checkpoint is
9c023f9 (enum bitfields as tagged-union selectors). Its design inferred an opcode
from a singleton enum in every complete command, which duplicated the public
header/opcode definition and gave `self.header.opcode` no clear outer owner.

The approved replacement distinguishes layout from selection. A generic untagged
union overlays complete records at byte zero, infers natural alignment/maximum
extent and never interprets a discriminator. CommandHeader and its opcode enum
are defined once and reused by all complete command records. PDMA's contract
interprets that common header to select a command. Copy's source/destination
remain existing tagged unions whose mode selectors belong to Copy itself.

A one-byte common header and one-byte Copy options allow sg_context@4, id@8,
length@16, payload contexts@24/28 and 32-byte endpoint unions@32/64: a naturally
aligned 96-byte command slot without parent-relative layout or split child records.
All addresses/counts/indices/offsets/lengths retain their widths. This is not a
claim of measured throughput improvement. No singleton-selector inference,
implicit opcode decoding, parent-tag lookup or old compatibility wrapper belongs
in the new generic untagged union. Tests must cover explicit variant selection,
wrong/unknown command opcodes, truncation, inactive storage and C wire agreement.

### Codec helpers versus protocol ownership

A proposed union_codec! binding was discarded before commit after user review.
An untagged union does not know which protocol information selects an interpretation,
and some users need no selection at all. Generic schema should not require an
opcode mapping DSL to provide useful union access.

The chosen boundary keeps layout/raw storage and explicit case encode/decode
helpers in schema. Contract owns protocol dispatch and any extra consistency
checks through a direct `SchemaCodec` implementation. Optional mechanical
trait/view adapters and the generated from_bytes/encode_into conveniences delegate
to that trait, without declaring a selector, prefix or opcode table in schema.
Raw variant access and protocol-checked values access have distinct names so a
normal view does not bypass application checks. The earlier inherent-hook adapter
direction was removed when SchemaCodec became the common composition boundary.

### Recursive codec composition

Review found that untagged branches still depended on record-specific inherent
methods and field metadata despite the existing layout trait. In particular, an
inner untagged union's opaque metadata caused valid nonzero bytes to be classified
as padding by its parent. This prevented the promised recursive composition.

`SchemaLayout` and `SchemaCodec` generalize the former record-only contract:
composition uses layout-associated Value, geometry and views plus codec methods.
Generated records implement both; an externally implemented struct, enum or
separate codec marker can do so without inherent schema APIs. Metadata defaults to an opaque extent,
while generated records preserve their detailed metadata for exporters.

An outer union passes exactly the selected child's extent to that codec and checks
only its own remaining tail. The child's own padding hook preserves strict
complete-record branch checks without changing ordinary record decoding's existing
gap tolerance. Opaque codecs validate their internals themselves. Tagged fields
continue receiving their external discriminator from their containing record;
this does not manufacture a tagless decoder or move protocol selection into schema.

Coverage includes multi-level nonzero nested unions, manual structs/enums and
codec markers whose Value differs from Self, field and tagged-case composition,
outer-tail ownership, exact child slices and independent unchanged PDMA wire bytes.

### Generated layout and custom codec responsibilities

`SchemaLayout` owns Value/View/ViewMut, name/documentation, alignment/extent,
structural metadata, zero, checked views and direct-padding validation.
`SchemaCodec: SchemaLayout` contains only decode and encode. Generated records
implement both. Every generated untagged union implements SchemaLayout, including
raw unions without a codec; application implementations supply only the two codec
methods. The external opt-in still adds only convenience adapters.

Handwritten non-schema types implement both traits. Layout-only generic consumers
can borrow raw union views without selecting a codec. Composition that decodes or
encodes child values still requires SchemaCodec and supports Value distinct from
Self. Padding remains structural: generated record layouts check direct gaps,
opaque layouts default to no check, and application codecs validate their own
representations. No selector inference or wire-format change accompanies the split.
