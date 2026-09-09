# PDMA MODELING WORKSPACE

## Current state

This workspace is the starting point for a clean PDMA rebuild.
The old contract, device, host, and retired sync implementation were deliberately
removed from master after backing up the complete source snapshot:
`backup/pdma-before-rebuild-20260909` at
`51707fe1d38e261a188e1d0ac14243c0caf58127`.

No replacement PDMA implementation exists yet. Old interfaces, numeric register
layouts, and ABI compatibility are not requirements for the rebuild.

## Active building blocks

| Crate | Responsibility |
| --- | --- |
| hardware-interface-pipeline | Typed stages, composition, and driving policies. |
| hardware-interface-runtime | Memory backends, register access/observation, CPU-local state, interrupt dispatch, and trace storage. |
| hardware-interface-schema | Runtime-neutral record/register schemas, metadata, access, and exporters. |
| hardware-interface-schema-macros | Schema code generation and compile-time validation. |

Keep these crates and their tests intact. Reuse their existing capabilities in the
replacement model; do not introduce parallel composition or dispatch mechanisms.

## Historical references

`crates/scenarios` is preserved unchanged but excluded from workspace membership
because its PDMA dependencies have been removed. It is not part of current test
acceptance and cannot run standalone in this cleanup snapshot.
Reintroduce scenarios deliberately as the new model gains real behavior.

`include/pdma_model_abi.h` remains an unchanged historical generated artifact.
Its Rust generator lives on the backup branch. Do not hand-edit that header or
treat it as the replacement's ABI.

The documents under `model/docs`, including the previous architecture specification,
describe the backed-up implementation. They must not force old software interface
shapes into the rebuild.

## Implementation constraints

- The first replacement slice is one command queue, simple DMA copy, and IRQ delivery.
  Do not implement further slices without a user request.
- Queue state and its registers belong to the hardware object; actions compose through
  existing stages. Runtime assembly selects memory backend and execution policy.
- Keep the model deterministic and step-based. No cycle timing or extra executors.
- Software handlers remain outside the hardware. Preserve CPU-local and handler
  execution guarantees supplied by the runtime.
- Use native module declarations and matching paths. No `#[path]`, conditional path
  redirection, `include!` workaround, symlink workaround, or duplicated source trees.
- Tests use Given/When/Then and observable outcomes. Do not weaken them to get green.
- No unsafe code, production `unwrap`/`expect`/`panic`/`todo`, or unchecked indexing.
- Keep Rust source below 250 pure LOC; split by responsibility.
- Use existing Cargo/rustfmt/clippy settings and terse public API docs.
- Do not stage build caches or incidental agent state.
- Prefix every git command with `GIT_MASTER=1`.

## Verification

Run from `model/modeling`:

```bash
cargo fmt --all -- --check
cargo test --workspace
cargo test --workspace --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo doc --workspace --all-features --no-deps --document-private-items
```

The all-features pass exercises the generic C exporter. It stays default-off;
the removed PDMA contract no longer enables it through feature unification.
