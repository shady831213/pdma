# PDMA contract C headers

The contract generates one shared wire header and two static register views:

| Header | Register prefixes | Domain |
| --- | --- | --- |
| `pdma_common.h` | Existing record prefixes | Identical Host/firmware record sizes, encoding, enums and accessors |
| `pdma_host.h` | `C_HDMA`, `C_HBOOT`, `C_HCTL`, `C_HWIN` | Host |
| `pdma_firmware.h` | `C_FDMA`, `C_FBOOT`, `C_FCTL`, `C_FWIN`, `C_FCTX` | Firmware |

Both role headers include the common header and can coexist in one C11 translation
unit. There is no mixed full-access public header. The schema exporter generates
all record and register metadata from the original contract; PDMA's
`RegisterView` supplies only access restrictions. Projection retains physical
offsets, map extents, bank strides and instance indices. Hidden registers leave
holes. Read-only projections have zero writable masks. Descriptions are preserved.

| Register | Host | Firmware |
| --- | --- | --- |
| DMA discovery | Read-only, always available | Read-only |
| DMA global control and channel admin | Hidden | Original leaf permissions |
| DMA channel data | Original leaf permissions, dynamically gated | Original leaf permissions |
| Boot report | Read-only | Read/write |
| Window control | Hidden | Read/write |
| Window readiness | Read-only | Hidden (not mapped on management bus) |
| Control bootstrap / enable | PF only, read/write | Read-only |
| Control address-space attributes | PF only, read/write before enable | Read-only |
| Context table and maintenance | Hidden | Original leaf permissions |
| Request doorbell | PF only, write-only | Hidden |
| Response/event doorbell | Hidden | Write-only |
| Event delivery error | PF only, read-only | Read/write |

The role prefix is part of every register macro namespace. All offsets are relative
to platform-supplied bases; the contract exports no absolute base macros. Host and
firmware may use different bus views of the same hardware.

Each channel has an independent permission window. `ChannelWindowRegisters`
repeats `ChannelWindowBlock` with alignment four and inferred extent/stride twelve:
open at zero, firmware-only Function owner at four and status at eight. Both projections
retain 36 bytes for three channels. Firmware open offsets are 0, 12 and 24; owner offsets
are 4, 16 and 28; Host status offsets are 8, 20 and 32. WINDOW_COUNT and WINDOW_STRIDE describe this generated
geometry. There is no explicit bank stride or interleaving.

Static visibility never grants data access. Hardware checks the selected channel's
window on each transaction and rejects crossing accesses. Discovery remains
readable before boot; global and channel administration remain inaccessible to
managed Host even when all channels are granted. Leaf access rules still apply.
Firmware Ready opens no data window. Boot reporting and the independent control
transport remain available while data access is closed.

The checked-in headers select three channels: a 16384-byte DMA map with discovery at
zero and one contiguous administration block at four. Its global control registers
are at 4, 8 and 12; channel reset registers are at 16, 20 and 24. Data apertures
start at 4096, 8192 and 12288, each owning a complete 4096-byte page. The page size
is MMIO layout geometry, not an assumption about the Host OS page size. If the
administration bank grows beyond the first page, schema placement moves the data
bank to the next aligned page. Host projection hides administration
without compressing addresses or extent. Direct-DMA software using privileged
register access can use the full DMA definitions in the firmware header; the Host
header specifically describes the managed Host view.

Reset remains a write-one/request, read-busy handshake. Retain memory until hardware
clears busy after old work drains. Managed Host uses ResetChannel/ReleaseChannel
requests instead of writing administrative registers. Platform-pending notifications
survive reset. The control map is 36 bytes: bootstrap at zero, enable at eight,
address-space attributes at 12 and request doorbell at 20. The firmware-owned
notification block starts at 24, containing response doorbell at 24, event doorbell
at 28 and event error at 32. Response and event remain independent notifications.
Enabling locks both bootstrap and address-space attributes. The latter select
Default or PASID independently of the data context table; no table slot is reserved.
There is one PF control block, not a per-Function bank. Static Host metadata describes
its PF permissions; hardware rejects every VF access. No control stride is exported.

Shared wire records: Segment is 16 bytes, Endpoint 40, Command 104, Response 72,
ControlRequest 32 and ControlResponse 24, ContextAttributes 8, ControlRingState 24,
ControlEvent 8 (alignment four), and ControlQueue 72. Enum bitfield constants are
unshifted values. Endpoints use mode zero for linear spans and one for scatter/gather
lists; WORD0/WORD1 hold the Linear base/offset (WORD2/WORD3 reserved), or all four
words hold SG list_address/count/index/offset. Each endpoint starts with mode/context
at 0/4 and parameters at 8. Command copy_length is at 88,
IOC at 96 and SG context at 100. Response IOC stays at byte 12. The nested source
and destination payload-context selectors are at bytes 12/52.
Response status selects an info union at byte 16. Success and stopping errors use
CompletionInfo (eight-byte bytes_completed); PageFault uses PageFaultInfo (56 bytes).
Its committed bytes/address are at 0/8, access/source-mode/destination-mode bytes
at 16/17/18, and source/destination position unions at 24/40. Linear stores offset
and a reserved zero word; SG stores index/offset. Access is SourceRead (1) or
DestinationWrite (2). The generated record-placement OFFSET/SIZE constants locate
info within Response; apply the selected case's generated accessors to that slice.
Inactive case storage is zero, and nonzero inactive storage is rejected by Rust.
These are deliberate wire-layout changes, not compatibility aliases. The channel's ring_context register selects one binding for both
DMA rings; source, destination and SG selectors independently choose data bindings.
ControlQueue nests command state at byte 0,
response state at byte 24 and event state at byte 48; each ring stores base/capacity/producer/consumer
at bytes 0/8/12/16. ControlRequest stores ID/opcode/target Function/channel/context
at bytes 0/8/12/16/20 and creation attributes at byte 24.

The firmware context map has capacity, one aligned 64-bit maintenance request
(context ID in bits 0..31, Drain/Abort action in bits 32..33), and a retained result.
Hardware never clears entry validity on completion; firmware owns that write.
After the entry bank, the final `failed` bank contains one 32-bit bitmap word per
32 hardware channels. Increasing channel count grows this tail without moving
earlier placements. Firmware reads the bitmap only after Failed; the next accepted
non-Idle request replaces the snapshot. See `docs/context-contract.md` for lifetime
and ordering requirements.

Initialize record arrays to zero before using generated stores so reserved padding
matches Rust encoding. Accessors encode bytes; they do not perform device I/O.

Run from `model/modeling`:

```sh
cargo run -p pdma-model-contract --features export-c-header --bin pdma-abi-header -- --channels 3 --contexts 3 --write-dir include
cargo run -p pdma-model-contract --features export-c-header --bin pdma-abi-header -- --channels 3 --contexts 3 --check-dir include
```

Both the explicit channel count and a directory action are required. The optional
context count defaults to the channel count for this export sample, not for hardware
construction. Generation
validates all three artifacts before writing; checking fails if any artifact is
missing or stale and never modifies files. Unsupported options and role selectors
fail before writing. `--help` lists the interface. Export remains default-off.

The strict C fixtures compile and run each role separately and both together with
`cc -std=c11 -pedantic-errors -Wall -Wextra -Werror`; wire bytes are compared
against independent fixtures and Rust encoding.
