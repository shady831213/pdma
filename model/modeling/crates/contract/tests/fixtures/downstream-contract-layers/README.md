# Downstream contract-layer fixture

Positive standalone consumer:

```text
CARGO_TARGET_DIR=/tmp/pdma-runtime-refactor-task11-target cargo run --manifest-path model/modeling/crates/contract/tests/fixtures/downstream-contract-layers/Cargo.toml --locked
```

Adverse direct-composition check (must fail because semantic `BufferId` is not an
ABI export and `AbiId` has no implicit semantic conversion):

```text
CARGO_TARGET_DIR=/tmp/pdma-runtime-refactor-task11-target cargo check --manifest-path model/modeling/crates/contract/tests/fixtures/downstream-contract-layers/Cargo.toml --locked --features negative-direct --bin direct-composition
```
