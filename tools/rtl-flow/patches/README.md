# Veryl patch provenance

Snapshot date: 2026-10-28. Upstream source:
[Veryl bacc4453](https://github.com/veryl-lang/veryl/tree/bacc44532af83058bdaab97dbe66ca1b89976be2).
Apply only in the order recorded in `../toolchain.env`.

- `veryl-guarded-read.patch`: upstream author commit
  `75e35bc59f6dcc69584474bf310f1639d4dca362`, submitted as
  [PR #3585](https://github.com/veryl-lang/veryl/pull/3585); proves guarded prior writes.
- `veryl-generic-parameters.patch`: a fix from
  [pcie_modeling](https://github.com/shady831213/pcie_modeling) for resolved generic
  module parameter forwarding and known literal interface widths.
- `veryl-generic-constant-context.patch`: a fix from the same project preserving
  originating namespaces for package constants introduced by generic substitution.
- `veryl-struct-member-width.patch`: PDMA's 2026-10-29 simulator fix. A struct
  constructor must resize each expression to its declared member width, including
  truncation; extension alone left an unsized-zero ternary at 32 bits when assigned
  to an 8-bit member and panicked in the real DMA frontend test. The fix reuses
  `resize_to_width` at that call site without changing global extension behavior.
  Regression cases cover ternaries, arithmetic, adjacent sentinels and X values.

The source project's qualification record for this snapshot reports 4075 passing
upstream workspace tests and 22 original ignored tests for the initial three-patch
manifest. That is inherited evidence, not a compiler-suite run performed by PDMA.
It also reports existing upstream Clippy findings; no all-clean Clippy claim is made.

Initial three-patch manifest SHA-256:
`fbfcb32cd20a0aa622a81bea6190a2444347926aeab65fc777dc9de1db769feb`.
The qualified Linux x86_64 dev binary used to seed the initial local installation:
`41d6c38f5a510164aba31f395870530031bd54613df9be689a4b161e8b97a7c5`.
This binary digest identifies that artifact, not byte-for-byte reproducibility
across build hosts. New builds record their own digest and provenance.

Current four-patch manifest SHA-256:
`f5013240f67e6549cb3eb6e972dbda10d15aa9755ef0f789edf4d27b64c57b7b`.
Current version: `0.22.0+upstream.bacc4453.guardfix.generics.context.structwidth`.
PDMA reproduced the width panic before the fix, then ran the full
`veryl-simulator` package suite: **1342 passed, 0 failed, 0 ignored**. The original
DMA test and both compiler smoke tests also run through the installed launcher
on CC and Cranelift backends. This is fresh simulator/downstream evidence, not a
fresh run of the entire upstream workspace or independent SystemVerilog simulation.
