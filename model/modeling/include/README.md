# Current PDMA contract header

`pdma_model_abi.h` is generated from the current contract's `Segment`, `Endpoint`, `Command`, `Response`
and `DeviceRegisters`, `WindowControl`, and `WindowStatus` schemas. It contains record sizes and byte accessors, MMIO
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

Commands are 64 bytes and carry `CompletionFlags.ioc`; responses remain 24 bytes
and echo the flags at byte 12. The checked-in header describes an explicitly
selected three-channel example. Its 228-byte MMIO map starts with read-only
`channel_num` at byte 0, followed by device IRQ enable, fatal state and reset at
bytes 4, 8 and 12. Three independent channel blocks start at bytes 16, 88 and 160
with a 72-byte stride. Each block contains command/response ring registers,
non-empty/IOC masks, read-only causes and channel reset.
Unmask does not replay notifications. Channel and device reset preserve external
memory; device reset also preserves the hardware channel count. Each device
instance selects its count at construction. The discovery register has post-reset
initialization metadata because hardware supplies that instance's count.

The management maps use separate `C_WINDOWCONTROL` and `C_WINDOWSTATUS` prefixes.
Their offsets are relative to `PDMA_WINDOW_CONTROL_BASE` and
`PDMA_WINDOW_STATUS_BASE`, respectively; these absolute base macros come from
the Rust contract. Both maps occupy four bytes. Firmware alone may access control,
whose writable bit zero opens the Host work window. Host readiness is read-only
and remains accessible while the work window is closed. These maps are separate
from the channel-count-dependent `C_PDMA_SIZE` extent.

Both reset registers are read/write handshakes. Write one to request reset, then
poll that same register until hardware clears it to zero. One means busy; zero
writes have no effect, and repeated one writes while busy coalesce. Hardware
rejects configuration writes for the affected scope until cleanup completes.
Completion guarantees that all prior command-ring reads, SG-list reads, payload
reads/writes and response-ring writes in that scope have drained and that no
later access belongs to that work. Software must retain affected memory until
completion, including when it cancels or times out its wait. Platform-pending
interrupt notifications remain claimable after reset.

Run from `model/modeling`:

```sh
cargo run -p pdma-model-contract --features export-c-header --bin pdma-abi-header -- --channels 3 --write include/pdma_model_abi.h
cargo run -p pdma-model-contract --features export-c-header --bin pdma-abi-header -- --channels 3 --check include/pdma_model_abi.h
```

`--channels COUNT` is required and selects the concrete instance metadata; with
no output option the generator writes to stdout. `--help` lists the options.
The optional `export-c-header` feature composes the existing schema exporters;
normal host/DMA builds do not enable it. Do not edit the header by hand.

Initialize record byte arrays to zero before storing fields so reserved padding
matches the Rust wire encoding. The generated field accessors do not initialize
padding or perform device I/O.

The obsolete header remains recoverable on `backup/pdma-before-rebuild-20260909`.
