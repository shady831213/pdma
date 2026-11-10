# Schema export design and review record

Current review candidate (unpublished): backends stream directly into caller-owned
`io::Write`. `prepare` returns `PreparedEntry` values exposing source name, identity
and `emit`, without filename or media-type policy. `OutputBackend` and
`OutputMetadata` have been removed. The temporary relative workspace schema patch
remains explicitly authorized for this review; standalone downstream fixtures keep
the published registry dependency. Do not publish the candidate without approval.

Schema and schema-macros 0.2.0 were published on 2026-11-05. Their reviewed snapshot
maps to `5a3a6f6cf13af855f3505968cfd21f601bfe83d7` after history cleanup; published
contents and checksums are unchanged. See [publishing.md](publishing.md).
Earlier qualification results below describe their own checkpoints and do not
qualify subsequent candidate edits. Direct streaming can leave partial bytes;
callers may stage output before publication.

## Decision and checkpoint

The 2026-11-02 review rejected a second representation of generated programs.
The implementation was restored to pushed checkpoint
`62c85cf670cdddd60ccc0c6132105c033c85673f` before this rewrite. Superseded local
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
                       writer -> application output policy
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
`5ca02bf983a2fcf04ea7757090697245419f2d7e` checkpoint before this revision.
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
| `entry(ctx)` | Entry callback for one selected tree |
| `layout_property` | A completed offset/count/stride/extent attribute |
| `register_map` | One map node, symbolic, bound, or explicitly projected |

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

Full AST and dependency resolution remain available to analysis passes. Renderers
receive shallow metadata, ancestor kinds, structural paths and bound visits;
reference callbacks receive the target explicitly. Register context supplies normalized register
location/offset/path and active unbound parameters. Original declarations and root
bindings remain separately queryable. These facts do not depend on visit order;
backend parameter collection and semantic scope reconstruction have been removed.

The common generate pass preflights the selected dependency closure for domain
compatibility before invoking callbacks or opening output. It emits in dependency
order, checking every entry's target name before the first write. `generate` accepts
an ordinary `io::Write`. `prepare` exposes the same bound entries for callers
selecting separate destinations; `prepare_entries` permits explicit domain
selection within a unified graph. `value::Backend::check_entry` defaults to no work;
targets use it for identifier and collision checks. A generator can share
`EntrySymbols` across domain backends. `PrepareEntries` integrates this preflight
with ordinary analysis passes. Each entry callback initializes its own render state.
Backends write incrementally; no final artifact buffer, `finish`, storage trait or
writer wrapper exists. Rendering errors may leave partial bytes. Opening, flushing
and publishing are caller I/O operations with ordinary I/O errors.
PDMA's RTL generator resolves one dependency graph and selects record/value roots
and register roots once. It checks the combined package namespace before writing,
then emits record/value packages followed by register packages, preserving each
group's dependency order. There is no heterogeneous package enum, identity map or
global interleaving restoration. No rendered-byte comparison or package-byte map
remains. CLI buffering of complete files remains application-owned, preserving
render-before-publish behavior. The schema library does not flush or publish output.

Default-off builtin features provide `CRecordBackend`, `CRegisterBackend`,
`VerylRecordBackend` and `VerylRegisterBackend`. C byte accessors use checked
storage metadata. Veryl packages describe layout, not bus behavior or hardware
register state. Custom documentation and dependency-graph outputs need neither
builtin. Target-private symbol state concerns spelling. Local syntax buffers may
arrange declarations and initialization statements from one traversal; they are
not another IR or additional generation passes.
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

The common generator dispatches register, map, placement, record and field nodes
to their semantic callbacks. Each owner exposes complete shallow metadata; width,
reset, access and other individual attributes do not need separate render methods.
Referenced register bits pass through the reference callback before value/bit-field
dispatch, retaining the containing register's access and encoding metadata.
Targets may order or buffer syntax sections without reinterpreting source kinds.

One `layout_property` callback receives the completed property kind and normalized
relative owner. Its expression is a bound child visit, formatted or evaluated by
the shared fold at the expression callback. Both builtins implement this callback
once. Veryl's register-name helper only
spells structural path segments; template/instance output policy is decided at
register entry. Target symbol
collision checks remain target responsibilities; output storage belongs to the caller.

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
come from common leaf callbacks over the already analyzed children.

The contract header generator selects record roots and separate Host/firmware
register projections. `contract::view::RegisterView` constructs complete DMA, boot,
control, window and optional context-table views; the generator does not implement
role permissions or bank-relative address mapping. Window count and stride use the
backend's ordinary bank constants, without additional handwritten aliases.
The RTL generator emits DMA record/value and register groups
separately, assigning shared definitions once before emission. Neither
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

## Full-crate audit corrections

The comprehensive audit after checkpoint `9c86f657` reproduced three correctness
defects: public ASTs could describe a field smaller than its actual representation;
following a record reference confused declaration attributes with outer placement
attributes; C scalar accessor names bypassed the final symbol collision guard.

Common validation now checks value extents, union case fit, bit shapes, enum and
constant ranges, and scalar byte-order consistency. Register source fields and
runtime definitions share a borrowed field validator with the same error ordering.
Packed records and explicit opaque storage remain supported. Attribute dispatch
uses its immediate semantic owner while preserving the full reference-site ancestry
for consumers. Generated C accessor functions share final-name checks with macros.

Concrete instance metadata is collected in one recursive walk. It retains the
existing checked geometry resolver, collects each register once and uses the
resulting ranges to populate banks. The separate register collection, string-prefix
bank scans and redundant final root extent resolution were removed. This is not a
claim that all recursive plan evaluation has been globally cached or eliminated.

## Canonical definitions and reference expansion

This direction was agreed on 2026-11-09 and implemented in the 2026-11-10 working
revision. Inline/Outline passes and their owned-map/register-bits alternatives are
removed. The macro construction syntax stays unchanged: the common graph builder
normalizes both macro output and manual input before semantic analysis.

- Construct one canonical AST of definitions and references. Builtin types such
  as u64 remain direct values; records, unions, bits and enums are defined types,
  including an enum whose representation is a builtin integer.
- Record whether each definition originated as a named or inline DSL declaration,
  together with source provenance. Inline origin is metadata, not a second AST
  representation or an instruction that every target must expand the definition.
- Remove the two optional structural rewrite passes. A reference callback chooses
  a named use or continues through the same backend and common traversal at a
  framework-bound expansion visit. Mixed expansion and named uses are supported;
  no copied all-inline tree or additional generation passes are needed.
- Rename the independent output entry callback from root to entry. Explicitly
  selected definitions remain valid entries even when referenced elsewhere.
  Definitions without incoming references may supply default entries, but
  containment parents and incoming definition references are distinct concepts.
- Expansion changes the current subtree, not the output entry. Common traversal
  retains the definition identity, occurrence identity, output owner, argument
  bindings, local layout and external selector bindings. It must not duplicate
  output wrappers or skip repeated occurrences through declaration deduplication.
- Keep backend callbacks consolidated. Do not add separate inline/named node
  callbacks, begin/end expansion hooks, output-phase backends, or backend-owned
  scope stacks and layout calculations. Generator entry selection stays outside
  node rendering; the revision must not add a backend dependency-planning protocol.

The default export entry helper selects named declarations in dependency order,
plus explicitly selected anonymous definitions. Generators can choose a subset
with `prepare_entries`; incoming references do not disqualify a definition from
being an output entry. Full AST traversal separately exposes all definitions.
This preserves dependency declarations without making backends plan dependencies.

Analysis prepares referenced-map geometry with actual arguments substituted while
retaining formal parameter metadata. Expansion does not unroll banks or invent
absolute register offsets. Defined enums inherit scalar byte order from their
record use site; physical storage projections retain their own encoding.

Qualification on 2026-11-10:
- The full all-features workspace run passed 1,516 tests with zero failures or
  ignored tests. After removing unused transition helpers, the schema/contract
  tests and workspace Clippy, strict rustdoc and formatting checks passed again.
- Regression coverage includes repeated and mixed expansion, explicit entries
  for referenced and anonymous definitions, packed enums, selector bindings,
  parameter substitution, local layout ownership and writer failures. Native C11
  checks include independently selected enum definitions and use-site byte order.
- Generated UVM, SV and Veryl files remain unchanged. The common C header only
  changes definition order: all 2,442 lines are preserved as a multiset; the host
  and firmware headers remain unchanged. All four generator check modes pass.
- Native UVM metadata checks pass for channel/bus-byte configurations (1,4),
  (3,4), (1,8), (3,8), and (1020,4). The generated inline-union SV consumer passes.
- The standalone renamed-dependency exporter fixture still uses published
  schema 0.2.0 and passes. No dependency declaration or macro package was changed.

Intermediate transform APIs and redundant source/selection helpers were removed;
source construction, analysis, rendering, tests and documentation were reviewed
together. Publication remains subject to separate user approval.

## 2026-11-10 interface consolidation candidate

- Map traversal has one `register_map` callback. The analyzed layout stays in
  analysis, and `layout_property` visits its offset/count/stride/extent children.
  Binding parameters no longer adds a second implicit concrete map sibling.
- Backends needing concrete addresses explicitly request `projected_children()`;
  bank instance expansion uses `instances()`. Source references, bank templates
  and permission projections remain separate, preserving sparse original indices,
  effective masks and hidden-address holes.
- Tagged-union physical fields are prepared during analysis at the selector's
  use site. Expanded repeated references reuse that projection while keeping
  distinct occurrence identities. Rendering does not scan parent offset ranges.
- Filenames, writer routing and MIME types belong to the generator. Target-name
  checks share `EntrySymbols` across record and register backends before writing.
  `ExpressionSyntax` stays independently reusable and is implemented directly by
  the builtin register backends and the application's UVM backend.
- The UVM value implementation lives in `uvm_regs/values.rs`; register structure
  and initialization remain under their existing owners. No new output protocol,
  generation scheduler or multiple generation passes were introduced.
- Canonical register enum references now reach C value callbacks; a strict C11
  consumer pins the discriminant declaration, alongside existing source/opaque,
  streaming-failure, projection and target-name checks.

Consolidation validation: default and all-feature workspace suites passed, with
no ignored tests, followed by focused tests for final naming/projection cleanup.
Strict workspace Clippy, private/public rustdoc and formatting checks passed.
C/Veryl/SV/UVM generation check modes matched the checked-in outputs. The Veryl
and UVM artifact updates change documentation only; executable declarations are
unchanged. Native C consumers, Veryl compilation, UVM register metadata profiles,
and 19 DMA VIP scenarios with seven deliberately invalid checker cases were
exercised. This is review evidence for the unpublished candidate, not approval
for a new crate release or full DMA RTL qualification.

## Register defaults cleanup and builtin-backend assessment

`register_field` now delegates to the shared value `bit_field` callback by default.
The render view normalizes the frontend's `u16`/`usize` IDs without narrowing; it
has no borrowed lifetime or generic ID parameter. Source AST and typed Rust-side
IDs remain unchanged. UVM and Veryl no longer implement duplicate field bridges;
UVM's identical `bits` override and the single-caller field wrappers were removed.
The regression covers both named bits and direct register fields, preserving the
register owner, reset/mask metadata and enum recursion.

`placement` already has a structural default. UVM deliberately selects references
there because it renders layout expressions and constructor arguments at their
actual initialization syntax sites. Its `layout_property` override opts into
expression output; changing the global default would leak bare expressions from
partial renderers. These overrides therefore remain target-specific, not generic
UVM policy added to schema.

### Feasibility and ownership

Both UVM-register and SV-record rendering are suitable optional schema backends.
Their renderer/codec/initialization helpers consume only schema metadata and std;
PDMA imports are confined to the application entry wrappers and integration tests.
No UVM simulator, VIP, jarvisuk or PDMA runtime dependency is needed by the Rust
backend. Proposed default-off features are `export-uvm-regs` and `export-sv-records`,
with separate `UvmRegisterBackend` and `SvRecordBackend` implementations.
These names describe a migration proposal, not features implemented or published
by this cleanup.

| Belongs in optional schema backends | Remains in contract / integration |
| --- | --- |
| UVM class/field/submap syntax and schema-derived initialization | Selection of DmaRegisters, Command, Event and other entry roots |
| SV byte-image types, sizes, offsets and endian-aware accessors | Hardware parameter bindings and Host/firmware permission policy |
| Target symbol validation, supported-input checks and writer errors | Package names, imports, file names, headers and writer publication |
| Non-PDMA schemas exercising backend capabilities | PDMA ABI oracles, UVM adapters, DUT/VIP assembly and tool invocation |

### Checks required before moving the current code unchanged

This is an architectural fit, not a conclusion that the current application
backends are ready for a generic release:

- SV target naming needs full collision checks. Entry preflight currently compares
  original names, but constants uppercase those names: distinct `Foo`/`FOO` entries
  both generate `FOO_SIZE`. Field/enum path flattening can likewise merge distinct
  names. UVM checks enum collisions but does not yet validate every emitted class,
  member and target-language identifier. Target checks must preserve the ability
  to use custom backends rather than constrain the source schema language.
- SV zero-sized records need an explicit supported representation or rejection.
  Current byte-image rendering emits `[SIZE*8-1:0]`; for zero this does not represent
  a zero-width image. This is an unsupported generic input, not a PDMA layout change.
- Numeric target widths need an explicit contract. UVM currently renders every
  unbound `usize` constructor parameter as 32-bit `int unsigned`; larger layout
  inputs can truncate even though layout arithmetic uses 64-bit literals. SV
  record size constants likewise use `int unsigned`. PDMA's current parameters
  fit, but a builtin must either preserve the source range or reject unsupported
  target sizes/bindings explicitly.
- UVM's current supported surface is little-endian source maps and fields with
  uniform access. Big-endian registers, partial writable fields and authoritative
  permission-only projections are explicitly rejected. A first builtin may retain
  these stated boundaries rather than imply broader support.
- UVM uses conservative volatile/no-mirror-comparison defaults and treats PostReset
  values as unknown. These choices need an explicit backend contract (and options
  if required), not inferred PDMA side effects. Bus width remains a caller-supplied
  build argument; hardware state and prediction policy remain outside schema.
- Migration must carry the non-PDMA tests, document raw byte-image versus typed
  codec semantics, and retain native SV/UVM qualification. PDMA wrappers should
  then contain only root selection and package/file assembly.

No backend migration, feature flag addition or publication is performed by this
assessment. The existing unpublished schema review and temporary override remain
in force.

Validation for the defaults cleanup: all-feature workspace tests passed 1523 cases,
default-feature tests passed 1404, with no failures or ignored tests. Strict
workspace Clippy, rustdoc and formatting passed. C headers, Veryl layouts, SV
records and the UVM model passed their check modes without changing generated
bytes. This validates the cleanup; it does not close the generic builtin-readiness
findings above.
