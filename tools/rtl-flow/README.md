# RTL tools

PDMA consumes the commit-pinned [my-veryl](https://github.com/shady831213/my-veryl)
submodule at `external/my-veryl`. Compiler recipes, patches, installer and compiler
regressions belong there. PDMA owns only the recipe selection in `toolchain.env`,
a forwarding launcher and its design/verification/implementation flows.

## Pinned compiler

Initialize the helper after cloning this repository:

```sh
GIT_MASTER=1 git submodule update --init external/my-veryl
bash tools/rtl-flow/veryl.sh --version
export PATH="$(bash tools/rtl-flow/veryl.sh --print-bin-dir):$PATH"
```

The selected recipe is `0.22.1-generic-wide-struct`: upstream Veryl 0.22.1 at
`8521c8c21e495916ca33648ddab746320953e11d`, Rust 1.97.0, locked Cargo dependencies
and `release-verylup`, with generic-parameter, wide-array and struct-member-width
fixes. This is the same compiler recipe as before extraction, not an upgrade to a
different fix set. The submodule commit pins its launcher and ordered patch bytes;
see its [provenance](../../external/my-veryl/PROVENANCE.md) and
[upstream tracking](../../external/my-veryl/UPSTREAM.md).

The upstream installer verifies source, patches and cached executable bytes and
serializes installation. PDMA defaults its cache to `tools/rtl-flow/.run/tools`,
so an already verified installation can be reused. `MY_VERYL_CACHE_DIR` overrides
that location. Compiler cache reuse is not a fresh source build. Prerequisites and
build controls belong to the [helper README](../../external/my-veryl/README.md).
The launcher neither changes the user's default Rust/Veryl nor simulator options.

Keep `VERYL_DUT_REUSE=0` explicit for native tests while upstream
[issue #3576](https://github.com/veryl-lang/veryl/issues/3576) remains unresolved
for this pin. Compiler regressions now live in the helper's `regressions/` native
projects; run those with the selected compiler on PATH. DMA's native tests remain
in `rtl/src/tests`. `node --test tools/rtl-flow/tests/veryl_test.mjs` checks PDMA's
forwarding boundary; installer integrity tests belong to my-veryl.

## Shared cells

`rtl/Veryl.toml` selects `common_cell` version `=0.2.0` and `tech_cell_generic` version `=0.1.1`
from [my-veryl-cells](https://github.com/shady831213/my-veryl-cells).
`rtl/Veryl.lock` pins both published source revisions. Veryl resolves their source
and package-owned constraints/checkers through native metadata. There is no
PDMA-local source copy or dependency override. Cell development and native cell
tests belong to that repository; PDMA qualifies their use in its own design.

An immutable snapshot captures emitted RTL, dependency metadata, package resources,
the compiler repository revision, recipe/patch content hashes and design input
hashes. Generic IP transport does not apply waivers just because a dependency
exists. Updating a dependency or compiler pin requires downstream qualification;
old source-bound reviews cannot silently cover new inputs.

## Contract layout

`bash tools/rtl-flow/layout.sh --write` runs the contract's `pdma-rtl-layout`
binary and writes `rtl/src/dma/layout.veryl`. `--check` rejects absent or stale
output without overwriting it. The generators consume registry schema 0.3.0
without a local override. Generated packages use `#[fmt(skip)]`; handwritten RTL
uses normal Veryl formatting.

`bash tools/rtl-flow/sv-layout.sh --write|--check` handles the plain-SV counterpart
at `verification/dma/tb/generated/register_layout.sv`. Its generator uses schema's
optional `export-sv` backend and preserves 64-bit layout parameters. `records.sh`
uses the same feature for byte-record types; `registers.sh` uses `export-uvm-regs`
for RAL. These outputs serve independent consumers.

## Maintenance boundary

Never fix a compiler defect by hand-editing generated SystemVerilog or weakening
a DUT assertion. Submit reusable cell/compiler fixes to their owning repositories,
then deliberately update PDMA's pins after verification.

Design-specific testbench and implementation logic belong in `verification/dma`
and `impl/dma`. Generic implementation adapters stay under `implementation/`;
[package resources](ip/README.md) use native Veryl dependency metadata. These flows
remain tracked; see [flow configuration](flows.md). Keep machine-local settings
in a private deployment file, and raw tool reports, proprietary inputs and
credentials outside tracked content.
