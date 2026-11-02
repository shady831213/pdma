# Schema AST and emit review candidate

Status: unpublished 0.2.0. Publication requires renewed user approval.
Normal consumers keep crates.io dependencies. Candidate checks use the explicit
temporary Cargo-home patch below, never a default workspace override.

## Decision and checkpoint

Checkpoint `85bb069d9e4a551f229d7f2c87101aa0bae3af5f` on
`schema-export-review` was committed and pushed to origin and github before this
rewrite. The user approved replacing the old export implementation outright.

The earlier export architecture duplicated target traversal and then replaced it
with a flat begin/end callback protocol. That removed repeated traversal but
scattered each structure's rendering and encouraged callback forwarding layers.
The accepted design treats schema as a declaration language with compiler-style
analysis and emit phases.

```text
existing DSL parse structure
    |-- macro expansion --> ordinary Rust API / codec / accessors
    `-- macro expansion --> schema AST construction
                                 |
                         shared analysis
                                 |
                          emit backends
```

The parser is unchanged. The AST construction branch consumes parsed DSL directly;
it does not parse generated Rust. Referenced Rust types and const expressions are
resolved by normal Rust compilation. The generated access/codec branch remains
independent of the emit target.

## Canonical declarations and dependency graph

`hw_model_schema::ast` replaces the former `graph` namespace without aliases.
Definitions retain original records, logical fields, bits/enums, unions/cases,
registers, maps and placements. Physical encoding attributes coexist with source
structure: inline unions are not represented as synthetic word fields. Inline
cases contain their original fields; external codec cases reference named types.
Constant constraints retain their source expression and evaluated wire value.
Records retain declared versus inferred alignment and source endian.

`Definition::node()` yields a borrowed `Node`. `children()` describes
containment; `references()` finds named definition dependencies recursively
inside that containment. Sorting collected child views does not change source
order. Nodes are not an exporter-specific copy of schema metadata.

`SchemaSet` combines manual roots and optional discovery. Dependency resolution
uses identity-based visited/active state to deduplicate definitions, detect cycles
and produce dependency order. Distinct fields/cases referencing one definition
remain distinct edges. Explicitly named instances are not deduplicated as repeated
members. Rust type identity is process-local and is not a wire identifier.

Inline unions/cases no longer implicitly add compiler-generated helper types as
dependency definitions. Their structure stays in containment. Ordinary Rust helper
types/codecs still exist as before; explicit selection remains possible. Generated
artifacts therefore drop those synthetic standalone definitions while retaining
the actual named dependencies and layouts.

## Shared analysis

`ast::analysis::Analysis` retains canonical declarations and attaches additional
checked layout results. It does not duplicate scalar declarations into another
type classification. Source field bounds and overlaps are checked independently
of physical storage layout. Record storage resolution still owns codec offsets
and endian/width behavior.

`Bindings` supplies values for selected unreferenced map roots. Named instances
retain separate identities. A shared generic definition cannot simultaneously be
specialized under that same identity; use a named instance or separate request.

`MapLayoutPlan` owns parameter binding, referenced-child geometry, checked
arithmetic, alignment, noninterleaving, offset/count/stride and extent.
`LayoutExpr` is the source expression algebra. Numeric evaluation, validation and
analysis use its recursive fold. After binding, analysis lowers the plan into a
`LayoutProgram` containing result members, parameters, required locals and ordered
statements. Child alignment/extent operands become literals or resolved symbols
before emission. No Geometry AST node family is introduced; the program is a
computed map attribute. Target structs/functions spell its already analyzed body.

Manual source construction uses `RecordDefinition` and the other AST constructors.
`Analysis::opaque(name, RecordSpec)` explicitly selects storage metadata when no
logical source declaration is supplied. It does not invent source fields.
`Analysis::register_map` accepts existing permission-projected resolved maps.
`MapView`/`PlacementView` borrow declarations or concrete projections of the same
map/placement kinds, preserving holes, bank identity, ancestry, effective access
and masks. Permission selection stays application-owned.

## Whole-node emit

`export::Backend` exposes complete node handlers such as `record`,
`register_map`, `register`, `field`, `union` and `case`.
A handler chooses where to emit child nodes using common `emit`/`emit_all`.
The immutable per-call `Scope` provides the enclosing unit, analysis and embedding
information; ordinary recursion supplies the stack. There is no begin/end event
stream, mutable render stack, or per-schema Render forwarding trait.

Expression syntax is a separate nonrecursive capability:
`export::expression::ExpressionSyntax` controls literals, references and folded
operators. `format_expression` recursively formats analyzed expressions without
consulting source placement rules. `ProgramSyntax` supplies syntax for child calls,
local definitions and assignments; common `emit_body` handles statement order and
expression formatting. Whole-map handlers own target-specific structure/function
syntax. C/Veryl formatters do not select child calls, reconstruct cursor updates,
compute placements or recursively match expression trees.

The builtins are `CHeaderBackend` and `VerylBackend`, gated by their existing
default-off features. C byte accessors deliberately lower checked physical storage;
custom semantic backends can traverse original logical fields. Veryl emits layout
metadata, not hardware register state, bus behavior or side effects.

`generate(&analysis, backend, sink)` emits units in dependency order, calls
`finish` after all succeed, then passes owned artifacts to the sink. Analysis
and backend errors publish no artifacts. A sink error preserves previously accepted
artifacts; transactional storage rollback belongs to the sink. Errors remain typed.

The contract header generator selects record roots and application-owned
Host/firmware register projections. The RTL generator selects DMA roots.
Neither reconstructs layout formulas. The register listing and standalone DOT and
telemetry backends demonstrate user-defined output without builtin features.

## Candidate validation

From `model/modeling`, create a task-owned preview configuration:

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
RUSTDOCFLAGS='-D warnings' cargo doc --workspace --all-features --no-deps --document-private-items
cargo run -p pdma-model-contract --all-features --bin pdma-abi-header -- --channels 3 --contexts 3 --check-dir include
cargo run -p pdma-model-contract --all-features --bin pdma-rtl-layout -- --check ../../rtl/src/dma/layout.veryl
cargo run -p hw-model-schema --example register_listing
cargo run --release --manifest-path crates/hw-model-schema/tests/fixtures/schema-discovery/Cargo.toml
cargo test --manifest-path crates/hw-model-schema/tests/fixtures/downstream-exporter/Cargo.toml
```

Exit the preview shell before ordinary builds. Preserve desired logs, then remove
the task-owned preview directory when no longer needed. Do not publish from this
configuration or copy it into the repository.

Run native Veryl CC and Cranelift validation per `rtl/README.md` against regenerated
layout metadata. That validates the implemented DMA register slice only, not future
execution/drain integration, synthesis timing or physical CDC.

Required regression evidence covers original source hierarchy and constraints,
repeated edges versus unique definitions, cycles, source-order/sorted views,
projection permissions and nested-bank ancestry, typed errors/no partial output,
custom backend recursion and sibling-scope isolation, strict native C consumers,
actual generator freshness and both native RTL backends. Existing codec and wire
tests remain acceptance criteria. Compile-fail fixtures still reject invalid
types/alignments; their diagnostics may include the additional AST emission site.

Publication and the registry-only 0.2 transition remain deferred for user review.

## Paused checkpoint

Implementation and automated validation are complete for this revision; work is
paused at the user's request for review next week. No publication was performed.

- Workspace all-features: 1308 passed; default features: 1239 passed; no failures
  or ignored tests.
- Strict workspace Clippy, warning-free Rustdoc, Rust/Veryl formatting and both
  real generator freshness checks passed.
- Native Veryl CC and Cranelift each passed all 7 register-slice tests.
- Standalone custom backends, release/LTO discovery and native C consumers passed.
- Source-tree, recursion/scope, dependency, projection and manual source-field
  validation regressions passed. Source-field bounds had a captured failing test
  before the common-analysis fix.
- The 43 retained Veryl packages are unchanged; 11 synthetic inline union/case
  packages are no longer separate dependency artifacts. C/RTL outputs were
  regenerated and checked; Rust codec/wire and PDMA scenario tests remain green.

The old graph namespace, prepared-export schema, flat visitor/walker and begin/end
protocol are removed without compatibility aliases. Empty old module directories
were cleaned up. The next step is user review of the AST/analysis/emit boundaries,
not additional features or publication.

## Follow-up review (2026-11-02)

Resumed review found that `Analysis::record` validated a manual record but omitted
its referenced definitions and dependency edges. Generation could therefore emit
only the root, and cycles in referenced types escaped analysis. Three regression
tests reproduced these failures before the fix.

Manual records now collect dependencies through the existing `SchemaSet` resolver
and common analysis before appending the named root view. Shared definitions are
emitted once in dependency order, repeated field edges remain distinct, and cycle
errors retain their original path through `AnalysisError::Dependencies`. Both
manual and graph-selected definitions use the same reference-to-dependency
conversion. No second graph traversal algorithm or backend workaround was added.

Follow-up inspection covered AST macro emission, containment/reference traversal,
backend scope recursion, C/Veryl emitters and removed export interfaces. No further
confirmed issue was found in that scope. Candidate validation passed:

- Schema all-features tests: 339 passed, none failed or ignored.
- Contract all-features tests, including generator/native C checks: 89 passed,
  none failed or ignored.
- Strict schema Clippy, warning-free schema Rustdoc and workspace formatting.
- Actual header and RTL-layout generator freshness checks; no generated-file changes.

These are scoped follow-up checks, not a new full-workspace or native Veryl run.
The previous checkpoint's broader results remain recorded above. The candidate
remains unpublished and ready for user review.

## Layout emission consolidation (2026-11-02)

User review identified remaining duplicated computation orchestration in both
backend `geometry.rs` modules. Those modules are removed. Each backend now has a
`map.rs` that spells the analyzed result structure, function signature, locals
and statements. Generated public `Geometry`/`geometry` names remain output API,
not schema node kinds.

`ast::analysis::program` owns result members, child calls, local requirements,
statement order and resolution of source layout operands. `MapLayout` contains
the analyzed program rather than exposing a plan for each backend to interpret.
The original map declaration still provides `layout_plan` for checked formula
inspection and evaluation. `export::program::emit_body` visits the body once and
formats its expressions through shared recursion; target syntax implementations
cannot accidentally change placement order by reconstructing the calculation.
Obsolete backend layout-error variants and the C layout-error conversion were
removed. Formula failures belong to analysis; target naming/formatting failures
remain backend-owned.

Existing native C/Veryl layout tests passed before the change. New tests first
failed because common analysis did not expose an executable layout body, then
passed after lowering was added. They cover child-result dependencies and bound
programs without child calls or cursor work. Native C consumers verify symbolic
and bound results independently. Symbolic C now separates cursor declaration
from initialization, without changing its interface or numerical results.
The current PDMA header and RTL layout files remain byte-for-byte fresh.

Final validation: workspace all-features 1313 passed; default features 1244 passed;
no failed or ignored tests. Strict workspace Clippy, warning-free workspace
Rustdoc and formatting passed. Native Veryl CC and Cranelift each passed all
7 register-slice tests. Final targeted expression/program tests passed after lint
cleanup. Boundary review found no remaining backend layout interpretation; the
new computation and emission modules are each below 100 pure lines. The candidate
remains unpublished pending user review.
