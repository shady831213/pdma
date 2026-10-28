# Veryl patch provenance

Snapshot date: 2026-10-28. Upstream source:
[Veryl bacc4453](https://github.com/veryl-lang/veryl/tree/bacc44532af83058bdaab97dbe66ca1b89976be2).
Apply only in the order recorded in `../toolchain.env`.

- `veryl-guarded-read.patch`: upstream author commit
  `75e35bc59f6dcc69584474bf310f1639d4dca362`, submitted as
  [PR #3585](https://github.com/veryl-lang/veryl/pull/3585); proves guarded prior writes.
- `veryl-generic-parameters.patch`: pcie_modeling's local fix for resolved generic
  module parameter forwarding and known literal interface widths.
- `veryl-generic-constant-context.patch`: pcie_modeling's local fix preserving
  originating namespaces for package constants introduced by generic substitution.

The source project's `docs/evidence/port-completion-2026-10-27.md`, section
“项目工具修复与正式入口验证”, records 4075 passing upstream workspace tests and
22 original ignored tests for this exact three-patch manifest. That is inherited
evidence, not a compiler-suite run performed by PDMA. It also records existing
upstream Clippy findings; no all-clean Clippy claim is made.

Ordered patch manifest SHA-256:
`fbfcb32cd20a0aa622a81bea6190a2444347926aeab65fc777dc9de1db769feb`.
The qualified Linux x86_64 dev binary used to seed the initial local installation:
`41d6c38f5a510164aba31f395870530031bd54613df9be689a4b161e8b97a7c5`.
This binary digest identifies that artifact, not byte-for-byte reproducibility
across build hosts. New builds record their own digest and provenance.
