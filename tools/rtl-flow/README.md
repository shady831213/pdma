# RTL tools

PDMA maintains its own fixed Veryl installation. `toolchain.env`, the ordered
`patches/` and `veryl.sh` are a snapshot of
[pcie_modeling's qualified tooling](https://github.com/shady831213/pcie_modeling),
with installation paths localized to this directory. Running them requires no
pcie_modeling checkout. Shared repository extraction is deferred; future updates
are deliberate PDMA changes, not automatic tracking of that project's work.

## Pinned compiler

The initial upstream revision is `bacc44532af83058bdaab97dbe66ca1b89976be2`, built
with Rust 1.96.0 and the upstream Cargo.lock. The source archive and ordered patch
manifest have SHA-256 pins in `toolchain.env`. This is a locally patched compiler,
not an official Veryl release. Patches cover guarded-read analysis, generic
parameter forwarding, cross-project generic constant context and simulator
struct-member width conversion. See
[patch provenance](patches/README.md).

The launcher checks patch integrity on every invocation, downloads and verifies
the source archive when needed, applies patches with zero fuzz, then builds and
atomically installs under `.run/tools`. `provenance.txt`, `PATCHES.sha256` and
`SHA256SUMS` accompany the executable. Later invocations verify the installed
binary without rebuilding. Concurrent installations serialize through `flock`.
Do not substitute a source Git checkout for the archive: preserved generated
parser files and timestamps avoid an upstream parser-regeneration problem.

Prerequisites: Linux x86_64, Rust/Cargo 1.96.0 for building, a C compiler/linker,
Bash, curl, tar, patch, coreutils and flock. `VERYL_BUILD_JOBS` defaults to four;
Cargo cache/target overrides and normal proxy environment variables are honored.
No machine-specific proxy or license setting is embedded. The user-level Veryl
installation is untouched.

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

For this initial setup, an independently checksummed copy of the already-built
qualified compiler can seed the local ignored installation. That is artifact
reuse, not a claim of a fresh local build. An absent installation triggers the
reproducible source-build path above.

The current four-patch installation was built through this source-build path.
The struct-width fix has a failing-first upstream simulator regression and is
also exercised by `smoke/src/struct_width.veryl` and the real DMA frontend test.

## Contract layout

`bash tools/rtl-flow/layout.sh --write` runs the contract's `pdma-rtl-layout`
binary and writes `rtl/src/dma/layout.veryl`. `--check` rejects absent or stale
output without overwriting it. The exporter uses the normal Cargo workspace and
published schema dependency; it introduces no local framework overrides.
Generated packages use `#[fmt(skip)]` so formatter alignment does not change the
generator's deterministic output. Handwritten RTL uses normal Veryl formatting.

## Maintenance boundary

An upgrade must update source/patch identity together and rerun relevant compiler
regressions and downstream positive/negative tests. Never fix a compiler defect
by hand-editing generated SystemVerilog or weakening a DUT assertion.

Design-specific testbench and implementation logic belong in `verification/pdma`
and `impl/pdma`. EDA integration remains deferred; this change does not copy
another project's flows, waivers or qualification claims. Keep machine-local
settings in ignored `local.env`, tool artifacts in `.run/`, and credentials/license
details out of tracked files. A future shared tool repository should replace this
tool directory without changing design source ownership or native manifests.
