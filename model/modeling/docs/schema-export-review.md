# Schema export review candidate

Status: unpublished 0.2.0 review candidate. Publication requires renewed user
approval. Normal consumers retain crates.io dependencies; review checks use an
explicit temporary Cargo-home patch, never a checked-in default override.

## Shared generation contract

`export::PreparedSchema` is the public, target-independent rendering input.
`PreparedSchema::from_graph(&graph, &bindings)` validates layouts and prepares
definitions in dependency order. `Bindings::bind::<Map>(&arguments)` specializes
selected map roots; an empty `Bindings` keeps parameterized roots symbolic.
A root also referenced as a generic dependency must be specialized in a separate
generation request. `PreparedSchema::record` and `::register_map` accept explicit
resolved views; register permission projection remains application-owned.

Named instances selected through `SchemaSet::add_map_instance` are prepared after
their reusable definitions, preserving separate view identities and arguments.
`SchemaKind` retains original record/union/opaque distinctions and external union
selectors alongside normalized storage metadata.

Every builtin and custom renderer implements the same `Renderer` callbacks.
`begin_definition(DefinitionHeader)` supplies identity/name/kind/documentation;
`reference` supplies one dependency edge. Record, bit/enum, map, bank and register
callbacks receive one semantic node at a time. Geometry callbacks receive folded
operands and validated symbolic references. Default callbacks allow a consumer to
select only the events it formats. `finish(self)` returns owned artifacts.
`record_member` preserves declaration order including nested containers, while
field/enum and placement callbacks support their respective output sections.
Expression operator and reference formatting are overridable; default C-like
syntax is shared, and custom syntax retains the common recursive fold.
`generate(&prepared, renderer, &mut sink)` owns all child traversal and emission;
renderers do not iterate schema children or recurse through expressions. Rendering
or common-layout failure emits nothing. `ExportError::Layout` preserves common
expression failures independently of `Render`; sink errors retain their original
typed cause and do not roll back artifacts already accepted.

The two builtin targets are `CHeaderRenderer` and `VerylRenderer`, behind their
existing default-off features. Both consume the same public prepared metadata and
layout representation. This unpublished candidate replaces the former generic
exporter interface and separate record/map exporter entry points without
compatibility aliases. Applications select and prepare input, configure target
names, invoke `generate`, then store the returned artifacts through their sink.

## Ownership and entry points

- `hw-model-schema/src/graph`: definition identity, references, root selection,
  dependency order, optional discovery and separately named map instances.
- `graph/map/expression`: one public `LayoutExpr` algebra for arguments and
  geometry. `LayoutExpr::visit` owns recursive traversal through `ExpressionVisitor`;
  numeric evaluation and constructor-scope validation use the same fold. Binary
  callbacks receive folded children; `Value(PlanValue)` is rejected in constructor
  scope. Backends do not contain another expression visitor or arithmetic evaluator.
- `graph/map/plan`: symbolic geometry, checked child classification and argument
  validation. `MapDefinition::layout_plan()` is fallible even without concrete
  arguments; invalid child kinds, arity, unknown parameters and cycles are rejected.
  The evaluator owns alignment, offset, count, stride, end, cursor and extent rules.
  `MapLayoutPlan::bind` uses its checked intermediate results to produce constant
  expressions and fixed child envelopes while retaining definition identity.
- `hw-model-schema/src/export/schema`: common preparation of records, registers,
  structural bank topology, symbolic plans and concrete instances. Renderers use
  prepared references and child names/identities instead of invoking raw definition
  callbacks. `SymbolScope` provides shared collision bookkeeping; target escaping
  and naming rules remain backend-owned.
- `hw-model-schema/src/export/visit` and the common walker: definition/member,
  field/enum, bank/instance and geometry callback ordering. Renderers own syntax,
  identifier rules, section assembly and artifact boundaries only.
- `hw-model-schema-macros/src/internal/expand/graph`: generated definition metadata.
  No catalog attribute, scope DSL or source scanner is required.
- `contract/src/bin/header/mod.rs`: selects protocol records and explicit Host and
  firmware permission views before common generation.
- `contract/src/bin/rtl_layout/mod.rs`: selects DMA roots and consumes generic
  Veryl artifacts without application address formulas.
- `examples/register_listing.rs` and `tests/fixtures/schema-discovery`: custom
  renderers formatting register and dependency-reference callbacks respectively,
  with no builtin backend feature enabled.

Discovery uses optional `inventory` registrations and remains independent of
manual selection. It sees compiled and linked definitions, including private
ones; selection is generator policy, not an access-control mechanism. Producers
must describe one deterministic definition per Rust type. The standalone discovery
fixture covers renamed facade imports, cross-crate unused definitions, repeated
references and release/LTO registration.

Version 0.2 is intentional: generated metadata calls require a matching facade.
The facade pins its macro version rather than permitting the published 0.1 facade
to resolve incompatible macros through its old caret dependency. Existing codecs
and ordinary Rust imports retain their responsibilities.

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

## Review and validation boundaries

Review the public prepared representation, shared child/layout validation,
parameter binding, dependency traversal, error atomicity and target naming.
Concrete C generation renders values from a bound plan; symbolic Veryl generation
renders formulas from the same plan. Custom renderers must not reconstruct those
rules. Existing record codecs, register access behavior and permission projection
remain independent of generation.

Veryl output describes geometry and metadata, not state, bus behavior or side
effects. Symbolic arguments use the portable arithmetic subset; arbitrary Rust
expressions are explicit preparation errors. Generated formulas assume valid
static parameters, so concrete instances must pass the common checked evaluator.
64-bit metadata does not expand the DMA RTL's supported register aperture.

The commands above define the review checks; historical test totals from earlier
exporter implementations are not evidence for this redesigned interface. Capture
fresh command output for workspace tests, Clippy, doctests, generator freshness,
standalone fixtures and native RTL runs. The independent default-feature listing
must still show `channels[0].status` at offset 0 and `channels[1].status` at offset
16. The discovery fixture must retain both repeated reference edges in its DOT
artifact. Strict C consumers and RTL tests validate the generated artifacts through
their real downstream surfaces.

Publication and the registry-only 0.2 dependency transition remain deferred until
user approval. After authorized publication, regenerate lockfiles without the
preview patch and verify resolved source IDs from PDMA roots. Do not use this
review configuration as the normal workspace build.

## Visitor revision review

Both builtin renderers now receive the same structural callbacks. The common
walker owns definition, member, field, enum, bank, parameter, placement and child
call ordering. Neither backend has a whole-definition render entry point or a
private expression walker. Target name preflight, documentation text and scalar
byte-accessor formatting remain target-owned.

Review also corrected grouping of nested bank instances and parenthesized Veryl
maximum expressions before their use as operands. Tests cover callback order,
custom expression syntax, enum emission counts, projected permissions, typed
errors and atomic artifact publication. Existing compile-fail expectations remain
unchanged. Source files in the shared exporter and both backends stay below 250
pure LOC. No production model behavior or protocol layout was changed.

Final validation of this visitor revision used the temporary preview configuration:

- Workspace default: 1233 passed; all-features: 1301 passed; zero failed or ignored.
- Workspace Clippy and Rustdoc with warnings denied, plus Rust/Veryl formatting,
  passed.
- Both real generators passed freshness checks. The visitor migration preserved
  all three C headers byte-for-byte relative to the preceding prepared-schema
  revision.
- Native Veryl CC and Cranelift each passed all 7 register-slice tests.
- Strict C consumers, the independent downstream renderer fixture, default-feature
  register listing and release/LTO discovery fixture passed.

All validation processes completed. The external preview cache and ignored evidence
logs remain available for review. No commit, push or publication was performed for
this revision.

## Backend callback consolidation

The follow-up cleanup removes private methods that merely mirror Renderer
callbacks. Veryl geometry is formatted directly in `veryl/visitor.rs`; its former
`geometry.rs` is deleted. C callbacks likewise own node formatting and section
boundaries. C's former bits/geometry helper modules are folded into naming and
scope handling. Shared helpers retain concrete responsibilities: typed constants,
symbol validation, register metadata and complete scalar byte accessors.
No new callback trait, dispatch layer or public API was introduced.

Cleanup validation passed 329 schema tests with all features, 10 contract
generator/C-consumer tests, strict schema Clippy and workspace formatting.
Both real generator freshness checks passed; all three headers and the Veryl
layout remain byte-identical to the preceding visitor revision. Existing native
RTL evidence applies to those unchanged generated bytes. Backend Rust files stay
below 250 pure LOC. This cleanup remains uncommitted and unpublished.
