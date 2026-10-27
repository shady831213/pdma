# Reusable library boundaries

The runtime extraction prepares independent libraries for publication while PDMA
remains one integration consumer. Package names retain the provisional
`hardware-interface-*` prefix. No package is published by this change; manifests
retain `publish = false` until names, ownership and release contents are approved.

## Ownership and dependencies

| Package | Responsibility | Normal dependencies |
| --- | --- | --- |
| storage | Closure-scoped local/shared storage with explicit reentry rejection | std |
| memory | Byte access, opaque sideband, backend errors, atomics, backing and allocator | storage |
| register | Register transport/addressing and ordered observations | storage; optional schema integration |
| irq | Source/readiness/claim/handler contracts | std; optional Tokio pending-interrupt backend |
| runtime | Software CPU, main/ISR scheduling, registry/dispatch, notifications and tracing | irq traits, register transport, Tokio |
| pipeline | Stepped stages, composition and synchronous/asynchronous drivers | std |
| schema | Layouts, codecs, register definitions and exporters | schema-macros |
| schema-macros | Schema parsing, checking and code generation | proc-macro libraries |

Here `std` means no third-party normal dependencies, not `no_std` support.
Register defaults to its `schema` feature; disabling it retains transports and
observations without schema. IRQ defaults to its `tokio` backend; disabling it
retains the source and handler traits. Remaining runtime functionality is software
execution, so the former `async-runtime` switch is removed. Hardware imports memory
directly. Moved APIs have one defining crate and no runtime compatibility reexports.

## Interface generality audit

Memory sideband remains caller-selected and opaque. Backend errors need not be
cloneable or implement `std::error::Error`. Ordinary accesses are byte-order-neutral;
atomic32/64 deliberately operate on naturally aligned little-endian words. That
documented subset is not arbitrary atomic byte-order support. Typed helpers operate
on byte buffers, not Rust object layout or schema codecs. The provided allocator is
a concrete monotonic shared-store allocator; no speculative allocator trait is added.

Independent IRQ delivery requires neither SoftwareCpu nor an ISR registry. The
former ordering requirement on every envelope's vector came from the registry's
BTreeMap; `Ord` now applies only where runtime lookup needs it. A plain source with
a non-ordered notification key can be claimed and passed to a handler. Asynchronous
traits still require Send where documented; a local-only IRQ framework is not added.

CPU affinity applies only to explicit CPU-local resources and capabilities.
CpuOwner remains private. Registry internals stay beside dispatch, while
CpuRegisterAccess consumes independent register transport. Splitting packages does
not expose token identity, mutex guards or scheduler internals.

Storage exposes synchronous scoped closures only. Blocking register observation
needs condition-variable waits, terminal poisoning and deferred faults. It now owns
private mutex/reentry bookkeeping instead of reaching into storage's private mutex
API. Existing nested access, unwind, emission-order and waiter-wakeup behavior stays.

Register transport supports its documented u32 window offsets and checked u64
address conversion. It does not infer device maps or side effects. Schema images
are optional, not a transport prerequisite. No speculative address-width or
synchronization policy framework is added during extraction.

Notifications are coalesced progress hints, distinct from non-consuming IRQ
readiness followed by claim. They and TraceBuffer remain small runtime services;
no additional one-module packages or PDMA protocol abstractions are introduced.

## Verification and release boundary

Tests move to their owning libraries. Mixed CPU/register/IRQ tests remain runtime
integration tests. The repository's native-module-path rule is enforced by PDMA
scenarios, not a distributable macro test scanning a consumer's filesystem.
Independent examples and downstream fixtures exercise custom errors, byte access,
IRQ delivery and composition. PDMA direct and managed scenarios retain behavior.

Local package verification must build the actual archive contents without the PDMA
workspace. Unpublished sibling dependencies may be patched to their extracted
archives for this check; this is not proof of their availability on crates.io.
Before upload, settle final names and owners, verify the declared minimum Rust
version with that toolchain, inspect package files/licenses and publish dependencies
in order. Repository relocation is separate from package naming/version continuity.
Actual PCIe/MMU/RTL features and new PDMA behavior are outside this extraction.
