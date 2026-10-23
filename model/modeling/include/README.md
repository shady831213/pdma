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
zero, atomic capabilities at four and administration at eight. Global control registers
are at 8, 12 and 16; channel reset registers are at 20, 24 and 28. Data apertures
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
survive reset. The control map is 36 bytes: its configuration block contains
bootstrap at zero, address-space attributes at eight and enable at 16. The request
doorbell remains at 20. The firmware-owned
notification block starts at 24, containing response doorbell at 24, event doorbell
at 28 and event error at 32. Response and event remain independent notifications.
Enabling locks both bootstrap and address-space attributes. The latter select
Default or PASID independently of the data context table; no table slot is reserved.
There is one PF control block, not a per-Function bank. Static Host metadata describes
its PF permissions; hardware rejects every VF access. No control stride is exported.

Channel data groups configuration at offset 44 (enable and ring context), recovery
at 52 (fatal, fault status and resume), and interrupts at 64 (enable, cause, mask
and Trap ACK). Both ring maps and the 4 KiB channel stride are unchanged.
Channel data includes read-only 32-bit FAULT_STATUS (`FaultStatus.faulted`, bit 0)
and write-only 32-bit RESUME (`Resume.request`, bit 0). A write-one request clears
faulted and permits retained work to become eligible; it does not enable the
channel, clear errors or complete the command. Zero and requests while not faulted
do not prearm a later fault. Owner/window checks apply in the Host view. Reset
clears faulted and cancels parked work. Software must serialize repair/resume with
reset/release/reallocation and retire old fault handlers before reuse; these
registers provide no episode or stale-request check. Shared software offers
`faulted() -> Result<bool, _>` and `resume()` without a ticket argument. See
[pageable-copy recovery](../docs/page-fault-recovery.md) for ordering and lifetime.

Shared wire records: Segment is 16 bytes, Endpoint 40, Command 112, Event 24,
TimestampCommand 16 (64-bit target address and 32-bit context; command kind 8),
ControlRequest 32 and ControlResponse 24, ContextAttributes 8, ControlRingState 24,
ControlEvent 8 (alignment four), and ControlQueue 72. Enum bitfield constants are
unshifted values. Endpoints select Linear (zero) or ScatterGather (one). Their
context is at byte four and their mode-selected payload starts at byte eight.

Command stores ID at zero, kind at eight and a 96-byte payload at sixteen. The
nine kinds are Copy, Write, Fill, Wait, Atomic, Semaphore, Fence, Trap and Timestamp (0..8).
CopyCommand stores source/destination at 0/40, length at 80 and SG context at 88.
WordTarget stores address/context/width at 0/8/12. WordWrite adds value at 16.
FillCommand stores address/length/pattern/context/width at 0/8/16/24/28.
WaitCommand stores target/value/comparison at 0/16/24. AtomicCommand stores
target/order/operation/operands at 0/16/20/24; CAS operands contain compare/value
at 0/8, and other operands contain value at zero. Atomic has no software result
address. SemaphoreCommand stores target/operation at 0/16. Trap has no payload.
TimestampCommand stores its fixed-width destination address/context at 0/8.

Event stores ID/code/info at 0/8/16. Codes 1..8 select InvalidLength, ReadError,
WriteError, LengthMismatch, ListReadError, Unsupported, InvalidOperation and
ArithmeticOverflow. Zero is invalid: successful commands produce no event.
ErrorInfo contains the committed byte prefix. Recoverable PageFault produces no
event and waits for owner repair/resume. A full event ring is fatal only when an
error must be published. Fence writes software completion storage; Trap requests
notification without ring storage. There is no response or IOC compatibility API.

Discovery capabilities are read-only atomic32/atomic64 bits 0/1. Channel IRQ
causes are event_non_empty/trap/error/channel_fatal in bits 0..3; only the first
two have mask bits. TRAP_ACK is write-only bit-zero acknowledgment at channel
offset 76; it does not retract a platform-pending notification. Each unmasked
Trap can emit, and enable/unmask never replays old notifications. Error stop is
latched before event publication and survives event consumption. The ring_context
register selects one binding for command and event memory; data selectors remain
independent. PF ControlResponse and ControlEvent are separate and unchanged.

ControlQueue nests command state at byte 0,
response state at byte 24 and event state at byte 48; each ring stores base/capacity/producer/consumer
at bytes 0/8/12/16. ControlRequest stores ID/opcode/target Function/channel/context
at bytes 0/8/12/16/20 and creation attributes at byte 24.

The firmware context map starts with a 16-byte management block: capacity at 0,
result at 4 and one aligned 64-bit maintenance request at 8
(context ID in bits 0..31, Drain/Abort action in bits 32..33).
The 16-byte context entries begin at offset 16.
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
