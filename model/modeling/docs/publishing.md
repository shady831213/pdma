# Publishing the reusable libraries

The current facade release is **hw-model-schema 0.3.0**, edition 2024, with
**Rust 1.96** as its supported baseline and **MIT OR Apache-2.0** licensing.
It pins the unchanged **hw-model-schema-macros =0.2.0**; other reusable crates
remain at 0.1.0. Only the facade was uploaded in this release.
The six library manifests allow only crates.io; PDMA and standalone fixtures
remain unpublished applications.

## Schema 0.3.0 provenance and adoption

Release source commit: `6bd212bc53e21e994258a02d48e05f86af65b967`.
Published archive checksum: `6b6584298952ba4469915ddd422025759b9c1b69567061edad6985ff6b41b69d`.
The version bump reflects incompatible export-context/entry APIs, while declaration,
codec, register-access and wire behavior remain unchanged. SV record/register and
UVM backends are now optional builtins alongside C and Veryl.

The final gate passed 1545 all-feature workspace tests, 1407 default-feature tests,
562 archive tests, all 32 feature combinations, strict lint/docs and independent
exporter, access-tree and release/LTO discovery consumers. The macro implementation
was compared byte-for-byte with its published 0.2.0 archive; no macro update was needed.
The standalone exporter was migrated to restricted callbacks and a custom analysis
pass, retaining its original output and byte-layout assertions.

Publication used a clean source commit, normal Cargo package verification, and an
isolated package manifest with registry dependencies only. No `--allow-dirty`,
`--no-verify` or sibling override was used for the upload. The isolated upload has
no automatic Git VCS stamp; the source commit above records its provenance.
The registry download matches the registry checksum, and all source, tests,
examples, README and license files match the reviewed payload.

The temporary relative workspace override has been removed. PDMA and standalone
fixtures require 0.3.0 and resolve it through crates.io with checked-in checksums;
local library members remain separate maintenance packages. Future releases
still require explicit authorization.

## Schema 0.2.0 provenance

Reviewed source snapshot in the sanitized history:
`5a3a6f6cf13af855f3505968cfd21f601bfe83d7`. The 2026-11-06 history cleanup changed
commit identities and a reference path outside the published packages; both
package contents and crates.io checksums remain unchanged. The original history
and old-to-new commit map are retained in a private backup. Both packages passed
archive tests, all eight facade feature combinations, strict lint/docs checks,
native C/Veryl consumers and downstream discovery/exporter validation. Publication
used normal Cargo verification and the existing crates.io credential provider,
with no sibling patch, `--no-verify` or `--allow-dirty` option.

The workspace's application dependencies already required the unpublished schema
version. Cargo therefore rejected direct workspace publication during resolution,
before uploading anything. Publication proceeded from the verified package
directories outside that workspace, macros first, then facade against the newly
published macros. Source, tests, examples, README and license files were compared
byte-for-byte with the reviewed archives before upload. Cargo regenerated package
manifests/lockfiles; these standalone uploads have no automatic Git VCS stamp.
The reviewed source commit above records their provenance explicitly.

PDMA's dependency requirements were already 0.2.0 during review. Release adoption
updates the workspace and standalone fixture lockfiles to registry identities and
checksums; local maintenance members remain distinct from those dependencies.

## Release contents and order

| Package | Library surface | Published siblings required |
| --- | --- | --- |
| `hw-model-io` | Memory/register capabilities, optional byte backing, bounded allocation and ordered observations | None |
| `hw-model-irq` | Interrupt source/handler traits; optional Tokio interrupt wire | None |
| `hw-model-pipeline` | Continuation stages, composition, routing and executors | None |
| `hw-model-schema-macros` | Procedural implementation of schema declarations | None |
| `hw-model-schema` | Layouts, codecs, registers, definition graphs, passes and optional C/Veryl/SV/UVM backends | Exact compatible `hw-model-schema-macros` version |
| `hw-model-swenv` | SoftwareEnv, scoped main/ISR execution, notifications and tracing | `hw-model-io`, `hw-model-irq` |

If the macro implementation changes, publish it before its dependent facade.
An unchanged macro implementation stays at its already published compatible version. IO and IRQ must precede the software
environment. The first four entries are otherwise independent. The macro crate
is published for dependency resolution; applications use the schema facade.

## Validation before upload

Use a clean, reviewed release commit and a task-owned build directory outside
the repository. Keep the workspace lockfile unchanged during verification.

```sh
export CARGO_BUILD_JOBS=4
export CARGO_TARGET_DIR="$(mktemp -d /tmp/hw-model-release.XXXXXX)"
cargo test --workspace --locked
cargo test --workspace --all-features --locked
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
RUSTDOCFLAGS='-D warnings -D missing_docs' cargo doc --workspace --all-features --no-deps
cargo fmt --all -- --check
```

Run these commands from `model/modeling` on Rust 1.96. Also exercise the independent
downstream fixtures, inspect each `.crate` inventory and normalized manifest, and
check that licenses, README, examples and source are included. Public library
packages must not contain PDMA implementation packages, credentials or build output.
Cargo may omit nested fixture packages; validate those separately from the repository.
Remove the task-owned build directory after recording verification results.

Before a matching macro version is published, the facade cannot resolve that sibling
from crates.io. Local preparation may package with `--no-verify --exclude-lockfile`,
extract the **actual archives**, and patch siblings to their extracted locations
in a temporary workspace for builds/tests/docs. This checks the archives without
reaching back into the PDMA checkout. Use a fresh target after re-extraction:
archive timestamps are normalized and a reused target can retain stale products.
This local check does not prove that registry dependencies are available.

Put temporary sibling patches in the verification Cargo home's `config.toml`, not
only in a command-line `--config`: tests may spawn Cargo subprocesses, which need
the same resolution. Archive verification uses a separate Cargo home whose patches
point only to the extracted archives. Inspect `cargo metadata` to confirm that no
checkout source satisfies a package dependency. Do not copy credentials into these
verification homes. Never upload from a patched verification environment.

Once each prerequisite is published and indexed, run normal `cargo package` for
its dependents without sibling patches. Do not use `--no-verify` for the actual
upload. Registry availability and authenticated upload remain separate checks.

## Authenticated release

Confirm the intended crates.io account, verified email and publishing permission.
Configure credentials through `cargo login` or an approved credential provider;
never put tokens in this repository, shell command arguments or review logs.
Recheck version availability just before upload; preparation does not reserve a version.

Only after explicit release authorization, publish a schema release from the
reviewed commit in order:

```sh
# Only when the pinned macro implementation has changed:
# cargo publish --registry crates-io -p hw-model-schema-macros
# Wait for any new prerequisites to become available before dependents.
cargo publish --registry crates-io -p hw-model-schema
```

If unpublished application dependencies prevent workspace resolution, use the
verified package directories as isolated manifests with `--manifest-path` instead.
Compare all shipped source with the reviewed commit, use an unpatched Cargo
environment, and retain normal package verification for each upload. Record the
source commit separately when the isolated package has no Git VCS metadata.

If an upload fails, stop and check which versions actually reached the registry;
resume from the first unpublished package rather than republishing a completed one.
Afterward verify versions, owners and docs.rs builds, then smoke-test a fresh
consumer with registry dependencies only. Record the release commit and tag only
after confirming the intended release state; do not mark this candidate released early.

PDMA's GitHub repository became public on 2026-11-06. A public Git repository is
not a prerequisite for publishing a crate: the uploaded archive must be
self-contained, while links to a private source repository require access to it.

References: [Cargo publishing guide](https://doc.rust-lang.org/cargo/reference/publishing.html)
and [manifest publish setting](https://doc.rust-lang.org/cargo/reference/manifest.html#the-publish-field).

For 0.3.0, post-publication source/checksum checks covered all seven PDMA roots
and all three standalone consumers. Registry-backed all-feature/default workspace
regressions and strict Clippy passed, and every generated artifact passed its
check mode. The versioned docs.rs Rustdoc pages, including SV and UVM APIs, are
available. The integration checkpoint is tagged `hw-model-schema-v0.3.0`.
