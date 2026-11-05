# Schema export review candidate

Status: unpublished 0.2.0. Publication requires renewed user approval. Normal
consumers keep crates.io dependencies; candidate checks use an external Cargo-home
patch configuration, never a checked-in default override.

Next user review: inspect the C header backend implementation, including record
accessors and register metadata/naming. The Veryl backend has been inspected by
the user; this checkpoint does not authorize publication.

## Decision and checkpoint

The 2026-11-02 review rejected a second representation of generated programs.
The implementation was restored to pushed checkpoint
`165158f60b9c165782f17628b69613f2ea7ff699` before this rewrite. Superseded local
changes were retained in a local stash for recovery, not used as the new design.

There is one semantic schema representation. Its contents describe records,
registers, fields, maps, banks, parameters, arguments, constraints, documentation,
dependencies and mathematical layouts. Functions, local variables, assignments,
returns and byte-access algorithms are target implementation choices. They do
not belong in a common schema IR. The previous `LayoutProgram` and `ProgramSyntax`
interfaces have been removed.

```text
existing DSL parser
    |-- macro expansion --> Rust API / codec / accessors
    `-- macro expansion --> semantic schema declarations
                                      |
                     resolve -> validate -> layout -> simplify
                                      |
                              AnalyzedSchema
                                      |
                              generate(backend)
                                      |
                       artifacts -> application sink
```

Macro expansion constructs metadata from the parsed DSL. It never parses the
generated Rust. Existing Rust codecs and accessors remain independent of export.

## One semantic representation

`ast::Node` groups record, register and shared value domains. Each domain has a
typed node view. Record fields and union cases retain their source hierarchy;
physical storage words do not replace logical fields. Registers retain width,
access, reset, writable masks and fields. Register maps retain declarations,
parameters, placement arguments, bank identity and alignment/extent constraints.
Bits and enums share source definitions while domain backends can render them
differently.

`children()` describes containment. `references()` preserves named dependency
edges, including repeated references from distinct fields. `SchemaSet` combines
manual roots with optional discovery. Identity-based resolution deduplicates
definitions, detects cycles and orders dependencies. Named instances remain
separate from their generic definitions. Rust identity is not a wire identifier.

The analysis pipeline is organized under `src/analysis`, independently of source
AST and export. `pipeline.rs` explicitly composes resolve, validate, layout and
simplify modules. Adding or splitting a pass changes that sequence and its
contracts, not target backends. Passes use ordinary Rust functions rather than
a registration framework or dynamic scheduler.

`AnalyzedSchema` has private construction and immutable unit access. The normal
`analyze` entry starts at a source selection; `from_graph` accepts an already
resolved graph. Both run the remaining passes before producing the completed
result. Changes to source declarations require reanalysis. Concrete permission
projections enter through an already checked register-map type.
All constructors, including manual records, opaque envelopes and checked register
projections, pass their complete unit set through the same finalization function.
Adding a final analysis or optimization therefore also covers manually supplied roots.

The layout pass resolves child arguments, sizes and ordered placement dependencies.
The simplify pass folds constants and safe arithmetic identities. Its completed
`CompleteMapLayout`/`CompletePlacementLayout` attributes contain expressions
evaluable using only root parameters. `PlanValue` and `ChildLayout` remain source
analysis vocabulary and are never interpreted by target backends. Source
declarations, argument names, documentation and references survive binding.

Constraints retain positive counts, alignment, fit and nonoverlap obligations.
Constant contradictions are rejected by analysis; parameter-dependent constraints
remain queryable and are checked by layout evaluation. Generated layout helpers
assume valid parameter arguments. Simplification must preserve arithmetic errors;
it cannot discard an invalid operand merely because another operand is zero.

`LayoutExpr` represents mathematical expressions and references to layout
properties. Shared evaluation handles checked arithmetic, alignment, child
geometry and noninterleaving. Shared expression formatting folds operands through
the target's nonrecursive `ExpressionSyntax`. The target decides whether formulas
become constants, functions or temporary variables. No C or Veryl constructs are
introduced into the common representation.

`AnalyzedSchema::record` resolves referenced definitions for manually constructed
records. `AnalyzedSchema::opaque` accepts physical storage metadata without inventing
logical fields. `AnalyzedSchema::register_map` accepts existing permission-projected
maps, preserving holes, ancestry and effective access. Permission policy remains
application-owned.

## Independent record and register emission

The C-specific two-view experiment was rejected and restored to the pushed
`6a36d025b98598ae2ea4c59290c387ba0e4b966b` checkpoint before this revision.
Record emission continues to use the source AST nodes. No public RecordLayout /
RecordMember traversal or layout-selection mode is added.

Optional work before generation is explicit Rust composition:
`Pipeline::new(input).run(pass_a)?.run(pass_b)?`.
`Pass<Input>` owns its output and error types; the required Resolve, Validate,
LayoutSchema and Simplify stages use the same protocol. Custom passes can run
between stages, including before layout. Validation/layout states are immutable;
consuming `into_source()` discards derived state and requires validation and layout
again after source transformation. Results may borrow source metadata directly.
For target preparation, a pass can carry `(&schema, data)` into the next pass.
That pipeline's `generate(factory, sink)` borrows the resulting data. Existing
simple generation needs no Pipeline wrapper or target-specific pass.

Target-neutral `traversal` is available to passes independently of a Backend,
including pre-layout source traversal. Explicit
reference traversal retains the selected root and the actual occurrence's ancestor
chain; a reusable definition has no single mutable parent. Complete paths and
derived offsets can therefore be computed by generator-defined passes. The C
offset pass lives with the C backend and is selected by the header generator;
public core passes do not acquire C naming, codec operations or offset policy.
Opaque storage and tagged-union word projection remain explicit C representations,
without inventing source AST fields or routing ordinary records through a second
flat traversal.

The public names distinguish the source model, analyzed input and traversal role:

| Name | Responsibility |
| --- | --- |
| Source `Definition` | A schema declaration in the source graph |
| Analyzed `Unit` | A completed definition or projected view; enumerated by `units()` |
| `NodeId` | Identity of roots, child nodes and reference-site occurrences |
| Traversal `Context` | Read-only node, ancestry, source/analysis and binding queries |
| `Visit` | A node bound to its context, for roots and descendants alike |
| `root(ctx)` | Entry callback for one selected tree |
| `layout_property` | A completed offset/count/stride/extent attribute |
| `map_layout` | The completed symbolic or bound map layout |

There is no separate public root-visit wrapper. The common generator validates all
units before the domain constructs their bound visits, then enters each root.
Targets dispatch the root's current visit through the same `emit` method used for
descendants. Source graph `definitions()` and target-specific C `_Geometry` names
retain their original meanings.

Use the independent `export::record::Backend` or `export::register::Backend`
callbacks with the single `export::generate` entry. Each backend implements shared
`export::value::Backend` using its own domain scope and associated `Domain` marker.
A record backend has no register callbacks, and a register backend has no record
callbacks. The domain provides typed dispatch without a caller-side adapter.

Whole-node handlers surround `self.emit(ctx.current())` or
`self.emit_all(ctx.children())` recursion with target syntax. Read-only emit contexts
carry the current node, enclosing ancestors and analyzed definition; recursive
calls supply the stack. A handler can filter or sort bound child visits without
changing their lexical context. Context and visit binding are
generator-owned: a backend cannot combine an arbitrary node or unit with a context
from another definition. There is no begin/end protocol or mutable traversal stack.

Read-only analysis, dependency resolution, ancestor and structural path queries
remain available to custom backends. Register context supplies normalized register
location/offset/path and active unbound parameters. Original declarations and root
bindings remain separately queryable. These facts do not depend on visit order;
backend parameter collection and semantic scope reconstruction have been removed.

The common generate pass preflights the selected dependency closure for domain
compatibility before invoking callbacks. It emits in dependency order and calls
`finish` before publishing artifacts. Analysis or rendering errors publish no
partial artifacts. Sink failures can retain already accepted artifacts;
transactional storage is the sink's responsibility.

Default-off builtin features provide `CRecordBackend`, `CRegisterBackend`,
`VerylRecordBackend` and `VerylRegisterBackend`. C byte accessors use checked
storage metadata. Veryl packages describe layout, not bus behavior or hardware
register state. Custom documentation and dependency-graph outputs need neither
builtin. Target-private text buffers are formatting details, not another IR.
Each register backend owns its map formatting callbacks directly. There is no
separate map renderer or forwarding entry point. Veryl formats parameter
signatures from the current context, typed attributes and expression spelling;
package output shared with record rendering remains separate. The obsolete scope
lookup for recovering map analysis was removed once callbacks received complete input.

Completed register traversal distinguishes mathematical map attributes from
concrete permission projections through typed callbacks. Backends never branch
on `Declared/Analyzed/Resolved` or retrieve a second view from scope themselves.
Common traversal supplies parameters, placement properties and concrete members;
targets control their textual representation and output order. Veryl no longer
needs child geometry calls or a cursor to resolve layout formulas.

## Leaf dispatch and documentation

The common generator, rather than each target, dispatches source attribute and
layout-property variants to typed callbacks. Register, map, placement, record and
field ownership is explicit in callback names. Register handlers traverse the
common children without rebuilding attribute arrays; referenced register fields
are exposed by the same traversal as inline fields. Targets may reorder children
for output format, but do not reinterpret their schema kind.

Default offset/count/stride/extent callbacks delegate to `layout_property`, which
receives the completed expression and normalized relative owner/property. Both
builtins implement this generic callback once. Veryl's register-name helper only
spells structural path segments; template/instance output policy is decided at
register entry rather than repeated by every attribute callback. Target symbol
collision checks and output buffers remain target responsibilities.

Common documentation dispatch preserves summary/detail distinction, explicit
empty text, effective placement fallback and source ownership. It invokes target
callbacks only for present metadata, in summary/detail order. Target comment
writers split and escape text; source lines do not become IR nodes. Normal C and
Veryl node generation uses these callbacks. The C record codec's physical-member
walk uses the shared fragment helper for the same optional-text behavior.

Veryl's former `Geometry` result structure and aggregate `geometry()` function are
removed. Each layout leaf emits its scalar function directly, retaining existing
offset/count/stride signatures and adding a direct extent accessor. The Rust-side
member/body collection is gone. C's aggregate `_Geometry` interface remains a
deliberate target format used by native consumers; its members and assignments now
come from common leaf callbacks rather than a second map traversal.

The contract header generator selects record roots and separate Host/firmware
register projections. The RTL generator selects DMA record and register roots,
merges identical shared value packages, and preserves dependency order. Neither
application generator computes layout formulas.

## Candidate validation

The optional-pass revision passed 1368 all-feature workspace tests and 1285
default-feature tests, with no failures or ignored cases. Strict workspace
Clippy, warning-free private Rustdoc, formatting, both real generator freshness
checks, standalone downstream exporters and release discovery passed.
The C source traversal preserves all three generated headers byte-for-byte.

Tests exercise independently implemented pass chains and a custom backend,
schema-borrowing outputs without cloned metadata, typed preparation failures,
discarding results on schema replacement, distinct occurrences of a shared
reference, opaque boundaries and domain-filtered traversal over mixed schemas.
The first mixed-domain register-root test exposed a missing filter; the final
implementation shares its supported-domain predicate with generation preflight,
and the full regression was rerun after that fix. Native C coverage includes
repeated nested records, bits, enum documentation, opaque children, tagged storage
words and untagged envelopes. The rejected second record-layout traversal was
removed before this revision and has not been retained as a compatibility route.

The naming and root-visitation revision passed 1350 all-feature workspace tests
and 1275 default-feature tests, with no failures or ignored cases. Strict Clippy,
private Rustdoc with warnings denied, formatting, both generator freshness checks,
standalone downstream consumers and release discovery passed. Generated C headers
and the RTL layout are unchanged by this revision.

Root and child nodes now share `Visit`; ownership compile-fail coverage remains,
including private root/context construction and raw-node rebinding rejection.
Behavior tests additionally cover dependency-root order, root dispatch through the
ordinary node path, exact ancestor chains and preservation of pre-existing sink
contents when rendering fails. C native consumers retain their exact wire/layout
checks. Backend modules are grouped under record and register ownership; C scalar
accessor generation belongs under record, while register naming and metadata
formatting belong under register. No compatibility aliases or empty source
directories remain from the replaced export names.

The readonly-context revision passed 1346 all-feature workspace tests and 1271
default-feature tests, with no failures or ignored cases. Strict workspace Clippy,
warning-free private Rustdoc, formatting, standalone downstream exporters and
release discovery passed. Both actual generator freshness checks passed without
changing the generated headers or RTL in this revision.

New regressions cover reordered siblings and grandchildren retaining their true
parents, parameter/binding queries before parameter visitation, independent bound
and unbound C map signatures, typed render failure without partial artifacts, and
compile-time rejection of scope/visit/root construction or raw-node rebinding.
Native C consumers execute the resulting geometry and codec interfaces; native
Veryl check/build accepts both symbolic and bound packages. Read-only structural
queries remain available to custom backends. The candidate remains unpublished.

From `model/modeling`, use a task-owned external Cargo configuration:

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

Exit the preview shell before ordinary builds. Preserve logs as needed, then
remove the task-owned temporary configuration and build directory. Never publish
from this configuration. Native Veryl CC and Cranelift checks follow
`rtl/README.md`; they cover the DMA register slice, not execution/drain integration
or physical timing/CDC.

Acceptance includes codec/wire tests, source hierarchy, symbolic and bound layout,
repeated dependency edges, cycle detection, projected permissions, domain
preflight, recursion/scope isolation, typed failure propagation, native C
consumers and both actual generator freshness checks. The source-tree test also
uses a documentation backend to observe parameters, arguments and attributes
without C/Veryl-specific intermediates.

## Prior semantic-IR checkpoint validation

Before the pass restructuring, the workspace all-feature run passed 1322 tests. The default-feature run
passed 1250 tests. Neither run had failures or ignored tests. Strict workspace
Clippy, warning-free private Rustdoc and formatting checks passed. Both actual
PDMA generators remain byte-for-byte fresh; the four C golden files are unchanged.
Native Veryl CC and Cranelift each passed the seven DMA register-slice tests.

Review caught two empty-map rendering defects: Veryl referenced an undeclared
cursor, and C emitted unused parameters/locals rejected by strict native compiler
flags. Both have failing-first regressions and target-local fixes. The final
all-feature run includes those regressions; native Veryl also compiled the fixed
empty-map output. No extra schema IR was introduced to solve either problem.

Removed interfaces, empty source directories and forbidden source redirection
were checked. PDMA hardware/software behavior was not changed by this rewrite.
The candidate remains unpublished for user review.

## Analysis-pass revision validation

The compiler-style pass restructuring is complete. Final workspace all-feature
tests passed 1328 cases and default-feature tests passed 1256 cases, with no
failed or ignored tests. Strict workspace/all-target Clippy, private Rustdoc with
warnings denied, Rust formatting and both actual generator freshness checks passed.
Standalone downstream exporters and release/LTO discovery passed.

The generated C headers and existing C goldens remain unchanged. RTL layout
helpers now contain completed formulas instead of child-layout calls and cursor
updates; their numerical layout is unchanged. Native Veryl CC and Cranelift each
passed all seven DMA register-slice tests. Separate symbolic, bound, empty and
nested-map native fixtures passed, and an intentional stride mutation was detected.

Review added backend-independent coverage for closed expressions, retained
constraints and metadata, statically false alignment despite unbound parameters,
and expression growth across 24 sequential placements. The latter produces 237
extent-expression nodes rather than exponential duplication. Manual roots and
projections now share finalization; dependency substitution and optimization have
independent visitors. No further confirmed issue remains in the reviewed scope.
No publication, commit or push was performed.

## Leaf-emission revision validation

The subsequent audit removed Veryl aggregate geometry and moved attribute,
layout-property and documentation dispatch into the common generator. It also
covered reusable bits definition text and effective referenced-register fields.
The emitted headers and RTL were regenerated: comment presentation and Veryl
helper structure changed, while numerical layout and existing property accessor
signatures remained intact.

Final workspace all-feature tests passed 1334 cases and default-feature tests
passed 1260, with no failures or ignored tests. Strict all-target Clippy,
warning-free private Rustdoc, both generator freshness checks, downstream
exporters, release discovery and formatting passed. Native CC and Cranelift
each passed seven DMA register-slice tests; direct scalar helpers also passed
native numerical and deliberate-mutation checks. Typed documentation failures
were verified to leave the artifact sink empty.

## Neutral traversal and composable-pass revision validation

The current revision separates traversal from target emission. Early source passes
and completed analysis share occurrence identities and lexical context. Record
reference expansions keep their actual parent sites. Register source nodes retain
their IDs when layout introduces derived nodes; derived IDs cannot alias source
nodes. IDs describe an immutable schema, not persistent handles across edits.

Resolve, Validate, LayoutSchema and Simplify implement the same typed Pass protocol
as custom analysis. Source transformations discard certified derived state and
must revalidate. C-specific offset analysis remains a selected backend pass,
indexed by NodeId rather than textual paths. Framework traversal owns bank
topology and effective permissions. Bank geometry uses the same expression
callbacks as other layout properties, without parallel numeric/expression fields.
Expression formatting reuses the semantic expression visitor.

Review also removed duplicate validation between layout and simplification,
obsolete forwarding types and old consumer imports. The complete regression found
and fixed a missing bank owner in layout-property context; coverage now checks its
parent, owner, property kind and evaluated value.

The final all-feature workspace run passed 1379 tests, with zero failed or ignored.
The schema run without default features passed 339 tests. Strict workspace Clippy,
warning-free private Rustdoc, formatting, both actual generator freshness checks,
standalone downstream exporters and release discovery passed. Generated C headers
and Veryl layout are unchanged. The focused native C suite also compiled and ran
generated consumers. The candidate remains unpublished and uncommitted for review.

## C register naming ownership

C register names now use the same direct structural-path approach as Veryl.
The preceding NodeId-table revision was insufficient: it retained global short-name
allocation solely to preserve old generated spelling. That compatibility constraint
has been removed. There is no naming preparation, occurrence scan, name table,
template lookup or automatic ordinal alias.

Analysis supplies normalized path components for banks, template registers and
expanded instances. Both backends consume those components directly. Adding a
sibling or projecting out another register cannot rename a surviving declaration.
C-specific differences are identifier escaping, a root prefix for the macro
namespace, deterministic local abbreviation for long paths and final C symbol
collision checking. Veryl uses its package namespace and target identifier checks.

The C path stem joins escaped component names and explicit instance indices. A
stem longer than 32 ASCII bytes becomes its first 15 bytes, an underscore, and the
16-digit uppercase FNV-1a64 digest of the complete encoded stem. This keeps deep
paths usable without consulting any other node. Digest collisions are not assumed
impossible; final emitted-symbol checking still rejects them. Root prefix overrides
are not silently abbreviated. The checked-in role headers and native consumers
use the new names; wire layout, offsets, access masks and enum codes are unchanged.
