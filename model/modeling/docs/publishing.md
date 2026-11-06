# Publishing the reusable libraries

The published schema release is **0.2.0** for both `hw-model-schema` and
`hw-model-schema-macros`, edition 2024, with **Rust 1.96** as its supported baseline
and **MIT OR Apache-2.0** licensing. Both versions were uploaded on 2026-11-05;
other reusable crates remain at 0.1.0 and are not part of this release.
The six library manifests allow only
the `crates-io` registry; the workspace default, PDMA packages and standalone test
fixtures keep publication disabled.

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
| `hw-model-schema` | Layouts, codecs, registers, definition graphs, passes and optional C/Veryl exporters | Exact matching `hw-model-schema-macros` version |
| `hw-model-swenv` | SoftwareEnv, scoped main/ISR execution, notifications and tracing | `hw-model-io`, `hw-model-irq` |

The macro crate must precede its facade. IO and IRQ must precede the software
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
cargo publish --registry crates-io -p hw-model-schema-macros
# Wait for prerequisites to become available in the registry before dependents.
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

The private Git repository need not be made public for Cargo to build an uploaded
archive. The archive itself must be self-contained; source links to the private
repository remain accessible only to authorized repository users.

References: [Cargo publishing guide](https://doc.rust-lang.org/cargo/reference/publishing.html)
and [manifest publish setting](https://doc.rust-lang.org/cargo/reference/manifest.html#the-publish-field).
