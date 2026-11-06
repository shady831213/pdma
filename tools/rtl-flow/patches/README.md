# Veryl patch provenance

Snapshot date: 2026-11-06. Upstream source:
[Veryl 8521c8c2](https://github.com/veryl-lang/veryl/tree/8521c8c21e495916ca33648ddab746320953e11d).
Tool inputs follow [pcie_modeling's reviewed snapshot](https://github.com/shady831213/pcie_modeling/tree/67989b3ebbdc8466e79ded2c65ad64c0da5585b0/tools/rtl-flow).
Apply only in the order recorded in `../toolchain.env`.

- `veryl-generic-parameters.patch`: preserves resolved generic module parameter
  forwarding and known literal interface widths on the new upstream revision.
- `veryl-wide-array-guard.patch`: the simulator selects valid wide-array storage
  before extracting a guarded element. An invalid/unknown index must not leave
  a scalar sentinel where a pointer to wide storage is required. See upstream
  [issue #3671](https://github.com/veryl-lang/veryl/issues/3671) and
  [PR #3672](https://github.com/veryl-lang/veryl/pull/3672). Test registration is
  adapted to follow the generic-parameters patch.

The old guarded-read, generic-constant-context and struct-member-width patches
are no longer part of this pin. The new upstream handles their exercised cases;
the PDMA struct-width smoke and register frontend tests remain regression oracles.
Old patch bytes and their original evidence remain in Git history.

Ordered patch manifest SHA-256:
`481f5b9ad45f88c9f0a867cf75ced2b470400e9cc0397b598b56a2fc13fa201b`.
Version: `0.22.1+generic-parameters+wide-array-guard`.
Build: Rust 1.97.0, upstream Cargo.lock, `release-verylup` profile.
The independently verified Linux x86_64 executable reused for qualification has
SHA-256 `da80c14bd2c989549ca17dfac05a7250682533b61aabe44b2a42a59200c562cc`.
This identifies the reused artifact; it is not a fresh PDMA cold build or a
claim of byte-identical builds across hosts.

PDMA validation: build, **7 DMA register tests** and **2 compiler smoke tests**
pass on each of CC and Cranelift, with `VERYL_DUT_REUSE=0`. Launcher tests cover
profile-specific cache selection, empty and nonempty manifests, changed patch
rejection and installed-binary integrity. This is downstream qualification,
not a fresh full upstream compiler suite or physical implementation signoff.
