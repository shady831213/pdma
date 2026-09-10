# Current PDMA contract header

`pdma_model_abi.h` is generated from the current contract's `Segment`, `Endpoint`, `Command`, `Response`
and `DeviceRegisters` schemas. It contains record sizes and byte accessors, MMIO
offsets and access/reset metadata, and schema descriptions as C comments. It is
a C11 data contract, not a callable device API or the predecessor's ABI.

Enum fields export named discriminant constants and variant descriptions. For a
bit field, these constants are unshifted values; its bit position determines
where the encoded value belongs within the containing scalar.

Endpoints use mode 0 for linear spans and mode 1 for scatter/gather lists. The
existing exporter names the two union storage words `WORD0` and `WORD1`: these
hold address/length for linear endpoints and list_address/count for list
endpoints. Software keeps lists and buffers stable until command completion.
The response carries a 64-bit `bytes_completed` count; transfer lengths have no
fixed hardware limit.

Run from `model/modeling`:

```sh
cargo run -p pdma-model-contract --features export-c-header --bin pdma-abi-header -- --write include/pdma_model_abi.h
cargo run -p pdma-model-contract --features export-c-header --bin pdma-abi-header -- --check include/pdma_model_abi.h
```

With no arguments the generator writes to stdout. `--help` lists the options.
The optional `export-c-header` feature composes the existing schema exporters;
normal host/device builds do not enable it. Do not edit the header by hand.

Initialize record byte arrays to zero before storing fields so reserved padding
matches the Rust wire encoding. The generated field accessors do not initialize
padding or perform device I/O.

The obsolete header remains recoverable on `backup/pdma-before-rebuild-20260909`.
