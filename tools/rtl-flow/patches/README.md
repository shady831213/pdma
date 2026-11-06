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

- `veryl-struct-member-width.patch`: restores the native IR fix from PDMA's
  previous pin. Struct assignment must truncate as well as extend each expression
  to its member width before concatenation. Upstream 0.22.1 handles literals but
  still leaves wider nonliteral expressions unchanged. A wide struct assigned in
  `always_comb` can then corrupt adjacent fields on both CC and Cranelift.
  `smoke/src/struct_neighbors.veryl` reproduces this; the earlier single-field
  `let` smoke did not detect it. The fix changes simulation lowering, not emitted
  SystemVerilog or DUT encoding.

The old guarded-read and generic-constant-context patches are no longer part of
this pin. Their original evidence remains in Git history.

Ordered patch manifest SHA-256:
`083c78440af0bd7085059e25000f1ab2b8a00bb6ed3954d94f9b3a92df5e7396`.
Version: `0.22.1+generic-parameters+wide-array-guard+struct-width`.
Build: Rust 1.97.0, upstream Cargo.lock, `release-verylup` profile.
The previous two-patch Linux x86_64 executable reused for qualification had
SHA-256 `da80c14bd2c989549ca17dfac05a7250682533b61aabe44b2a42a59200c562cc`.
This identifies the previous artifact, not the corrected three-patch compiler.
The launcher builds the corrected pin from the checked archive and records its
own executable digest; no byte-identical-build claim is made across hosts.

Original two-patch validation: build, **7 DMA register tests** and **2 compiler smoke tests**
pass on each of CC and Cranelift, with `VERYL_DUT_REUSE=0`. Launcher tests cover
profile-specific cache selection, empty and nonempty manifests, changed patch
rejection and installed-binary integrity. This is downstream qualification,
not a fresh full upstream compiler suite or physical implementation signoff.
