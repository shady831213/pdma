# PDMA contract C headers

The contract generates one shared wire header and two static register views:

| Header | Register prefixes | Domain |
| --- | --- | --- |
| `pdma_common.h` | Existing record prefixes | Identical Host/firmware record sizes, encoding, enums and accessors |
| `pdma_host.h` | `C_HDMA`, `C_HBOOT`, `C_HCTL`, `C_HWIN` | Host |
| `pdma_firmware.h` | `C_FDMA`, `C_FBOOT`, `C_FCTL`, `C_FWIN` | Firmware |

Both role headers include the common header and can coexist in one C11 translation
unit. There is no mixed full-access public header. The schema exporter generates
all record and register metadata from the original contract; PDMA's
`RegisterView` supplies only access restrictions. Projection retains physical
offsets, map extents, bank strides and instance indices. Hidden registers leave
holes. Read-only projections have zero writable masks. Descriptions are preserved.

| Register | Host | Firmware |
| --- | --- | --- |
| DMA registers | Original leaf permissions, dynamically gated | Original leaf permissions |
| Boot report | Read-only | Read/write |
| Window control | Hidden | Read/write |
| Window readiness | Read-only | Hidden (not mapped on management bus) |
| Control bootstrap / enable | Read/write | Read-only |
| Request doorbell | Write-only | Hidden |
| Response doorbell | Hidden | Write-only |

The role prefix is part of every register macro namespace. Absolute bases are
`PDMA_HOST_BOOT_BASE` and `PDMA_FIRMWARE_BOOT_BASE` (0xffff_ff10),
`PDMA_HOST_CONTROL_BASE` and `PDMA_FIRMWARE_CONTROL_BASE` (0xffff_fe00),
`PDMA_HOST_WINDOW_STATUS_BASE` (0xffff_ff08), and
`PDMA_FIRMWARE_WINDOW_CONTROL_BASE` (0xffff_ff00). Register offsets are
relative to these bases; DMA offsets start at zero.

Static visibility never grants Host work-window access. Hardware checks the window
on every Host DMA transaction. Firmware Ready does not open that window.
The boot report and control transport remain accessible while DMA access is closed.
The 20-byte control map retains bootstrap at byte 0, enable at 8, request at 12
and response at 16, even though each role hides one doorbell. A doorbell write of
one notifies its peer; zero is a no-op. Hardware stores no ring geometry or cursors.

The checked-in headers select three DMA channels: a 228-byte map with four device
registers at bytes 0, 4, 8 and 12, followed by 72-byte channel blocks starting at
16, 88 and 160. Both views retain read-only channel count and IRQ cause registers.
Unmask does not replay old events. Reset is a read/write handshake: write one,
retain affected memory, and wait for hardware to clear busy after old work drains.
Platform-pending notifications survive reset.

Shared wire records retain their original sizes and byte layout: Segment is
16 bytes, Endpoint 24, Command 64, Response 24, ControlRequest and ControlResponse
16 each, ControlRingState 24, and ControlQueue 48. Enum bitfield constants are
unshifted values. Endpoints use mode zero for linear spans and one for scatter/gather
lists; WORD0/WORD1 hold address/length or list_address/count. Command IOC is at
byte 56 and response IOC at byte 12. ControlQueue nests command state at byte 0
and response state at byte 24; each ring stores base/capacity/producer/consumer
at bytes 0/8/12/16.

Initialize record arrays to zero before using generated stores so reserved padding
matches Rust encoding. Accessors encode bytes; they do not perform device I/O.

Run from `model/modeling`:

```sh
cargo run -p pdma-model-contract --features export-c-header --bin pdma-abi-header -- --channels 3 --write-dir include
cargo run -p pdma-model-contract --features export-c-header --bin pdma-abi-header -- --channels 3 --check-dir include
```

Both the explicit channel count and a directory action are required. Generation
validates all three artifacts before writing; checking fails if any artifact is
missing or stale and never modifies files. Unsupported options and role selectors
fail before writing. `--help` lists the interface. Export remains default-off.

The strict C fixtures compile and run each role separately and both together with
`cc -std=c11 -pedantic-errors -Wall -Wextra -Werror`; wire bytes are compared
against independent fixtures and Rust encoding.
