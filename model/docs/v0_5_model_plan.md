# PDMA v0.5 Model Plan

## Purpose

v0.5 turns the completed v0 executable model from a flat crate into an architecture-oriented Rust workspace with explicit host, device, contract, and scenario boundaries. The work is behavior-preserving: every v0 scenario remains valid, while code ownership becomes visible in crate dependencies.

## Scope

v0.5 owns three changes:

- Split the current `pdma-modeling` crate into four crates: `pdma-model-contract`, `pdma-model-device`, `pdma-model-host`, and `pdma-model-scenarios`.
- Introduce a shared DMA contract vocabulary for DMA-visible objects and byte ranges that both host and device understand.
- Separate contract-level DMA data structures from the device-owned backing store that holds bytes in the model.

## Crate Boundaries

| Crate | Role | Depends On | Owns |
| --- | --- | --- | --- |
| `pdma-model-contract` | Shared soft/hard contract vocabulary | none | IDs, errors, DMA object/range types, descriptors, queue config, completion records, event records, resource config, trace event schema. |
| `pdma-model-device` | Hardware-behavior device model | `contract` | `PdmaDevice`, queues, rings, scheduler, engines, resource pool internals, trace sink/export, and device-owned byte backing store. |
| `pdma-model-host` | Software driver/SDK model | `contract`, `device` | `Driver`, command ID allocation, local slot accounting, submit helpers, completion/event polling helpers. |
| `pdma-model-scenarios` | System-level validation stories | `contract`, `device`, `host` | Existing v0 end-to-end scenarios and scenario helpers. |

Allowed dependency direction is `contract -> none`, `device -> contract`, `host -> contract + device`, and `scenarios -> contract + device + host`. Device must never depend on host or scenarios. Contract must never depend on device, host, or scenarios.

## DMA Contract Model

The contract crate models DMA-visible objects, not physical media. It should start with the minimum vocabulary needed to replace v0 `BufferId`-centric descriptors without adding translation, protection, or real address-space behavior:

- `DmaObjectId`: opaque shared object handle visible to host and device.
- `BufferId`: v0-compatible alias during migration, backed by `DmaObjectId`.
- `DmaRange`: `{ object, offset, len }` byte range inside a DMA-visible object.
- `DmaRangeStart`: `{ object, offset }` helper only where one command-level length applies to several operands.

Access or direction metadata is deferred unless a v0.5 migration step needs it to preserve an existing invariant. v0.5 must not imply permissions, IOMMU translation, coherency, domain placement, or physical address decoding.

The device crate owns the actual byte store. The current `Memory` object should become or be documented as a device backing store, such as `DeviceBufferStore` or equivalent, and must not move into contract.

## Non-Goals

- No cycle timing, RTL timing, frequency, power, or performance counter model.
- No real DMA address translation, IOMMU, cache coherency, or memory-domain placement.
- No descriptor byte layout, register offsets, C ABI, generated headers, or MMIO register map.
- No interrupt, reset, Linux DMAengine, DPDK DMAdev, or OS integration.
- No new PDMA operation semantics beyond the validated v0 behavior unless explicitly approved later.

## Migration Waves

1. Baseline lock: run current v0 validation before edits.
2. Workspace control plane: create a Rust workspace while keeping behavior green.
3. Contract extraction: move shared types and introduce DMA object/range types without backing memory.
4. Device extraction: move hardware-behavior state machine and backing store behind `pdma-model-device`.
5. Host extraction: move `Driver` into `pdma-model-host` while preserving the scenario-driving API.
6. Scenario extraction: move v0 end-to-end tests into `pdma-model-scenarios`.
7. Transitional cleanup: remove any temporary facade and stale `pdma_modeling` imports.
8. Final validation and review: run full workspace gates and verify crate dependency direction.

## Validation

Every wave must keep the model green. The final gate is:

```bash
cd model/modeling
cargo fmt --all -- --check
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo doc --workspace --no-deps --document-private-items
```

Additionally, every changed Rust file must remain at or below 250 pure LOC. Generated `target/` content must not be staged or documented.

## Completion Criteria

- The final workspace contains exactly the four v0.5 crates.
- All v0 behavior scenarios pass from `pdma-model-scenarios`.
- Contract contains shared DMA-visible data structures but no backing store.
- Device contains the byte backing store and all execution behavior.
- Host contains the software driver/SDK behavior and does not own device state.
- Documentation and AGENTS guidance describe v0.5 boundaries and validation commands.
