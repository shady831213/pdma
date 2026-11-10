# RTL tools

PDMA maintains its own fixed Veryl installation. `toolchain.env`, the ordered
`patches/` and `veryl.sh` are a snapshot of
[pcie_modeling's qualified tooling](https://github.com/shady831213/pcie_modeling),
with installation paths localized to this directory. Running them requires no
pcie_modeling checkout. Shared repository extraction is deferred; future updates
are deliberate PDMA changes, not automatic tracking of that project's work.

## Pinned compiler

The upstream revision is `8521c8c21e495916ca33648ddab746320953e11d` (Veryl 0.22.1),
built with Rust 1.97.0, the upstream Cargo.lock and `release-verylup` profile.
The reported version is `0.22.1+generic-parameters+wide-array-guard+struct-width`. The source
archive and ordered patch manifest have SHA-256 pins in `toolchain.env`.
This is a patched compiler, not an unmodified official release. Three patches
retain generic parameter forwarding, fix wide-array guarded indexing and enforce
struct member expression widths in the native simulator;
see [patch provenance](patches/README.md).

The launcher checks patch integrity on every invocation, downloads and verifies
the source archive when needed, applies patches with zero fuzz, then builds and
atomically installs under `.run/tools`. `provenance.txt`, `PATCHES.sha256` and
`SHA256SUMS` accompany the executable. Later invocations verify the installed
binary without rebuilding. Concurrent installations serialize through `flock`.
Do not substitute a source Git checkout for the archive: preserved generated
parser files and timestamps avoid an upstream parser-regeneration problem.

Prerequisites: Linux x86_64, rustup with Rust/Cargo 1.97.0 for building, a C compiler/linker,
Bash, curl, tar, patch, coreutils and flock. `VERYL_BUILD_JOBS` defaults to four;
Cargo cache/target overrides and normal proxy environment variables are honored.
No machine-specific proxy or license setting is embedded. The user-level Veryl
installation is untouched. The launcher selects `rustup run 1.97.0` only for the
compiler build; it does not change PDMA's Rust model baseline or default toolchain.
Install that compiler toolchain with `rustup toolchain install 1.97.0 --profile minimal`.
The cache identity includes the build profile so a development build cannot be
mistaken for the optimized installation.

From the repository root:

```sh
bash tools/rtl-flow/veryl.sh --version
export PATH="$(bash tools/rtl-flow/veryl.sh --print-bin-dir):$PATH"
export VERYL_DUT_REUSE=0
cd tools/rtl-flow/smoke
veryl build
veryl test
veryl test --backend cranelift
```

Keep `VERYL_DUT_REUSE=0` explicit for native tests while upstream
[issue #3576](https://github.com/veryl-lang/veryl/issues/3576) remains unresolved
for this pin. The launcher does not disable tests or change simulator defaults.
The smoke project checks compilation and native execution only; it is not DMA
RTL, a complete compiler regression or CDC/RDC qualification. Design commands
run separately in [rtl](../../rtl/README.md).

For this upgrade, an independently checksummed copy of the already-built
qualified compiler can seed the local ignored installation. That is artifact
reuse, not a claim of a fresh local build. An absent installation triggers the
reproducible source-build path above.

The original upgrade passed the two initial smoke tests and seven register tests,
but those cases did not cover a wide struct assigned in an `always_comb` block.
The Copy slice exposed adjacent-field corruption in that case; the member-width
fix is restored and `smoke/src/struct_neighbors.veryl` retains its failing input.
Run all compiler smoke tests on both native backends after any pin change.
Run `node --test tools/rtl-flow/veryl.test.mjs` from the repository root to check
launcher cache selection and rejection of modified patches or installed binaries.
These tests use temporary fixtures and clean them up automatically.

## Contract layout

`bash tools/rtl-flow/layout.sh --write` runs the contract's `pdma-rtl-layout`
binary and writes `rtl/src/dma/layout.veryl`. `--check` rejects absent or stale
output without overwriting it. The exporter uses the normal Cargo workspace and
schema dependency. The current unpublished render-API review uses the explicit
relative schema override authorized in the model workspace; it does not publish
that candidate or alter the other framework dependencies.
Generated packages use `#[fmt(skip)]` so formatter alignment does not change the
generator's deterministic output. Handwritten RTL uses normal Veryl formatting.

## Maintenance boundary

An upgrade must update source/patch identity together and rerun relevant compiler
regressions and downstream positive/negative tests. Never fix a compiler defect
by hand-editing generated SystemVerilog or weakening a DUT assertion.

Design-specific testbench and implementation logic belong in `verification/dma`
and `impl/pdma`. Their reproducible flows and generic constraints are tracked;
see [flow configuration](flows.md). Keep machine-local settings in a private
deployment file, tool artifacts in local run directories, and credentials/license
details out of tracked files. A future shared tool repository should replace this
tool directory without changing design source ownership or native manifests.
