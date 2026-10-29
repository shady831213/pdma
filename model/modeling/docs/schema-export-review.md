# Schema export review candidate

Status: unpublished 0.2.0 candidate. The user requested review before publication.
The pre-change RTL checkpoint is committed and pushed as `63c98c3a` and `1afebd4d`.
This schema change is checkpointed on `schema-export-review`; master remains at
the pre-change checkpoint until review and the publication/dependency transition.
No publish command has been run.

## Decisions and code entry points

- `hw-model-schema/src/graph`: definition identity, typed references, explicit
  roots, dependency closure/order, optional discovery and separate map instances.
  Existing codecs and ordinary Rust imports keep their responsibilities.
- `hw-model-schema-macros/src/internal/expand/graph`: generated metadata; tagged
  union details are under their existing tagged-union owner. No catalog/scope DSL.
- `hw-model-schema/src/export`: the existing public exporter/sink traits. Builtin
  C and Veryl backends remain optional features in this crate; custom consumers
  can render the graph without either builtin feature.
- `contract/src/bin/header/mod.rs`: discovers protocol records and preserves
  explicit permission projection for the Host and firmware register headers.
- `contract/src/bin/rtl_layout/mod.rs`: selects three DMA roots and collects the
  generic Veryl backend's artifacts. Application rendering/address formulas removed.
- `hw-model-schema/tests/fixtures/schema-discovery`: two independent crates,
  renamed schema dependency, unused-definition discovery under release/LTO,
  repeated references and a complete application-defined dependency-graph exporter.

`linkme` was tested first. Its generated section declaration fails this workspace's
`unsafe_code = forbid`. `inventory` passes the same lint without local exceptions;
its optional registration mechanics are isolated from manual graph collection.
Discovery sees compiled/linked definitions, including private ones; selection and
export visibility remain generator policy. Metadata producers are deterministic
and must describe one definition per Rust type. Instance arguments are separate.

Version 0.2 is intentional: the new macro emits facade metadata API calls. Publishing
it as 0.1.1 would let the already published 0.1 facade resolve an incompatible macro
through its old caret dependency. The new facade pins its matching macro version.
Consumer declarations still use registry versions, with no permanent path overrides.

## Reproducing checks before publication

Because 0.2.0 is not on crates.io, candidate checks need an explicit temporary
configuration. Run the following from `model/modeling` in a shell reserved for
this review. The normal workspace has no patch table. Clippy's child Cargo metadata
queries must inherit the same configuration, hence a temporary Cargo home rather
than only command-line `--config` flags.

```sh
schema_workspace=$PWD
schema_preview=$(mktemp -d /tmp/pdma-schema-review.XXXXXX)
mkdir -p "$schema_preview/cargo"
cat > "$schema_preview/cargo/config.toml" <<EOF
[patch.crates-io]
hw-model-schema = { path = "$schema_workspace/crates/hw-model-schema" }
hw-model-schema-macros = { path = "$schema_workspace/crates/hw-model-schema-macros" }
[build]
jobs = 4
target-dir = "$schema_preview/target"
EOF
export CARGO_HOME="$schema_preview/cargo"
cargo fmt --all -- --check
cargo test --workspace
cargo test --workspace --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo doc --workspace --all-features --no-deps --document-private-items
cargo run -p pdma-model-contract --all-features --bin pdma-abi-header -- --channels 3 --contexts 3 --check-dir include
cargo run -p pdma-model-contract --all-features --bin pdma-rtl-layout -- --check ../../rtl/src/dma/layout.veryl
cargo run --release --manifest-path crates/hw-model-schema/tests/fixtures/schema-discovery/Cargo.toml
```

Run the two native RTL backends as documented in `rtl/README.md` against the
generated source, separately from Rust. Reset/drain and execution coverage remain
bounded by the implemented register slice. No physical CDC or synthesis claim is
made by these checks. The original datatype/layout tests remain in place; two UI
diagnostic snapshots grow because the same rejected input now also reaches
metadata trait checks.

Exit that review shell before using the normal Cargo home again. Remove the
task-owned `$schema_preview` directory after preserving any desired logs. Do not
copy the preview patch configuration into the repository or publish from it.

## Backend boundaries for review

The C graph backend renders record/union/bits definitions; register maps use the
existing resolved-view exporter, so a graph traversal cannot bypass application
permission projection. The Veryl backend emits geometry and metadata, not hardware
state/effects. Map argument expressions support a documented portable arithmetic
subset; arbitrary Rust functions remain explicit unsupported metadata. Generated
Veryl geometry functions assume validated static parameters. Wide metadata does
not expand the DMA RTL's currently supported 32-bit register aperture.

## Pause checkpoint

Validated with the explicit preview configuration above:

- Workspace default: 1212 tests passed; all-features: 1267 passed, no failures or
  ignored tests in either run (Rust test totals, including doctests).
- Workspace Clippy with warnings denied passed. Rustdoc with warnings denied,
  all features and private items passed.
- Both generators' freshness checks passed; contract tests include the strict
  C11 header consumer and independent ABI layout assertions.
- Native Veryl CC and Cranelift: 7 design tests passed on each backend.
- Cross-crate discovery/custom DOT backend passed in release/LTO; the existing
  independent C exporter and access-tree fixtures each passed their consumer test.
- Default DMA dependency tree contains neither inventory nor Tokio.

Next session: review the public graph/instance API, the optional discovery
boundary, generated metadata completeness and backend ergonomics. The definition
graph and symbolic map evaluator are additions, not replacements for the existing
runtime layouts/codecs. Inspect `header/mod.rs` and `rtl_layout/mod.rs` as the two
real consumers. Publication and switching the normal build back to registry-only
0.2 resolution remain explicitly deferred until user approval. After publication,
regenerate lockfiles without the preview patch and verify source IDs from PDMA roots.
