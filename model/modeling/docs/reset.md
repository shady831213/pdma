# Reset contract

Channel and device reset separate a software request from hardware completion.
Completion is the memory-lifetime boundary: affected old work can no longer issue
a command read, SG-list read, payload read/write or response write. It is not a
promise that the old command completed successfully.

## Register handshake and scope

The channel and device `Reset` registers have the same read/write protocol:

| Action or observation | Meaning |
| --- | --- |
| Software writes one | Request reset of this scope. |
| Software reads one | Cleanup is pending or in progress. |
| Software writes zero | No effect; software cannot cancel or acknowledge reset. |
| Software repeats one while busy | Coalesce with the pending request. |
| Hardware clears to zero | Affected accepted work has retired and reset state is installed. |

The request write does not finish reset, even for an idle channel. Hardware must
step to perform cleanup. While busy, otherwise valid configuration/control/ring writes
are rejected with `Error::ResetInProgress`, apart from the reset protocol itself;
register reads remain available. Software may configure and enable the scope again
after it observes completion.

Channel reset clears that channel's configuration, ring pointers, fault, IRQ causes
and notification controls. It discards its buffered payload and unpublished response.
Peer channel state and accepted work remain intact, although cleanup of the shared
engine can delay peers. Channel reset cannot clear a device-wide fault.

Device reset subsumes pending channel resets, blocks new work on every channel and
waits for all affected cleanup. It resets every channel plus device-wide fault and
controls, preserving the instance's read-only `channel_num`.

Neither scope rolls back external memory writes already performed. Notifications
already handed to the platform remain pending until the platform claims them;
reset does not retract them.

## How the current model reaches completion

`Dma::command_loop()` composes
`AfterEach<Repeat<RunCommand>, ResetControl>`. `Fetch` gates resetting channels and
device reset before accepting commands. Accepted work records its channel and generation in
`Dma.active`; the command, SG-list, payload and response paths check reset before
further memory access. `RunCommand` alone clears `Dma.active` when the complete
pipeline terminates, covering success, reset and errors at any stage. Its retained
`Operation<S, Dma<M>>` discards child state before ownership is released, including
for custom child stages that do not clear their own state. Response publication
can precede that outcome by additional composition steps; ownership remains held during
those steps, so reset cannot acknowledge cleanup early. Neither response ports nor
`Resettable` release whole-command ownership.
`ResetControl` runs after every outer step,
including a pending step, and clears reset only once affected ownership has retired.
An unrelated active channel does not prevent an idle channel's reset from completing.

`MemoryAccess` calls are synchronous and finish within a hardware step. MMIO has
exclusive device access, so a reset request occurs between steps, not halfway through
a memory call. Thus the model has no bus transactions, outstanding read/write queues,
cycle timing or artificial drain delay. Cleanup retires retained execution state;
the completion guarantee covers every affected memory path.

Channel generation increments once for a new reset request. Repeated requests do
not advance it, and cleanup does not zero it. `ChannelTask<T>` retains the generation
so guarded custom stages can reject stale tasks even after reconfiguration. This
is an additional stale-task guard, not the reset acknowledgment predicate.

Custom command graphs must preserve the single shared execution path, wrap the
assembled pipeline in `RunCommand<S>`, and drive `ResetControl`. The child must implement
`Stage<Dma<M>, Input = (), Output = (), Error = Error, Wait = ()>`; its internal
composition and arbitration policy need not match the default `Pipeline`. Merely
dropping a software waiter or abandoning an accepted `Operation` does not flush
hardware. The complete-loop guarantee does not claim that arbitrary externally
held task objects have been driven to completion; custom stages must preserve the
reset guard before side effects.

## Software waiting and memory lifetime

Direct-DMA Host `Channel::reset(self)` and `Channel::reset_dma(self)` consume the driver,
write the request and repeatedly read the same reset register until zero. Reset waits
have no driver-imposed CPU affinity; the supplied bus enforces any resource-specific
access restriction. Busy polls cooperatively yield so the
independently driven hardware can progress. A successful return means hardware
completion was observed. Reuse requires fresh channel setup.

Caller deadlines and cancellation are software policy. Dropping the wait does not
cancel an accepted hardware request, and a timeout or failed poll does not establish
completion. Keep affected command rings, SG lists, payload buffers and response
rings valid until reset completion is observed. For device reset, this applies to
every channel. No timeout value is modeled as a hardware cleanup duration.

## Managed lifecycle authority

`DmaRegisters` separates read-only discovery, a contiguous `DmaAdmin` block containing
global controls and the channel-reset bank, and page-aligned channel data blocks.
Channel objects still own their reset state.
Managed Host cannot access the administration block. It submits `ResetChannel`
or `ReleaseChannel` to firmware; only firmware writes reset registers in this mode.
Direct-DMA assembly exposes the complete hardware map.

Firmware closes the selected data window before reset and awaits actual completion.
ResetChannel retains allocation and reopens disabled/unconfigured data access.
ReleaseChannel frees allocation and leaves access closed. Host must stop old ring/ISR
use and preserve memory until the successful management response, then construct
a fresh driver. Ordinary management processing is serial; peer DMA remains driven.
For future VF FLR integration, PF must first release all of the VF's channels and
collect successful responses, preventing new allocations until FLR completes.
The later FLR requires no second firmware cleanup: the PDMA application endpoint
may acknowledge immediately under that precondition. PCIe drain and controller
completion remain separate. Direct FLR without preparation is unsupported; there
is no automatic precondition check or cleanup fallback. See the
[VF pre-FLR contract](function-isolation.md#agreed-vf-pre-flr-integration-contract).
PCIe FLR remains unimplemented; this decision changes no current reset behavior.

## Real hardware comparison

In a bus implementation, the corresponding contract is to stop issuing new logical
requests, fulfill protocol obligations of already committed transactions, and drain
**all** affected outstanding transactions across the relevant ports and read/write
paths before acknowledging reset. Waiting for only the most recent response is
insufficient. This is the hardware interpretation of the memory boundary above,
not an outstanding-transaction mechanism implemented by this model.

- AMD AXI CDMA completes committed AXI4 transactions and flushes queued transfers
  during reset. See [PG034, CDMACR](https://docs.amd.com/r/en-US/pg034-axi-cdma/CDMACR-CDMA-Control-Offset-00h).
- AMD AXI VDMA completes all pending AXI transactions during a channel reset.
  See [PG020, Resets](https://docs.amd.com/r/en-US/pg020_axi_vdma/Resets).
- Altera mSGDMA's stop sequence stops new reads/writes, waits for `Stopped`, requests
  reset, then waits for `Resetting` to clear. See
  [Embedded Peripherals IP User Guide, Stop DMA Operation](https://docs.altera.com/r/docs/683130/25.3/embedded-peripherals-ip-user-guide/stop-dma-operation?contentId=tsouJyxHQSasRoJ1e93thw).
- AMD AXI MCDMA per-channel soft reset waits for EOF/TLAST. That stream/frame boundary
  differs from this model's memory-access boundary. See
  [PG288, Enable Soft Reset Per Channel](https://docs.amd.com/r/en-US/pg288-axi-mcdma/Enable-Soft-Reset-Per-Channel).

## Behavioral coverage

These tests specify the observable contract; this list is not a test-run report.

| Test source | Boundary covered |
| --- | --- |
| [`dma/tests/reset_protocol.rs`](../crates/dma/tests/reset_protocol.rs) | Visible busy state, rejected configuration, whole-command retirement after response publication/rejection, repeated/zero writes and device reset scope. |
| [`dma/tests/multi_channel/reset_boundaries.rs`](../crates/dma/tests/multi_channel/reset_boundaries.rs) | Reset across command/payload/response interactions, preserved peer work and fresh configuration. |
| [`dma/tests/multi_channel/scatter_gather_reset.rs`](../crates/dma/tests/multi_channel/scatter_gather_reset.rs) | SG access boundaries and absence of stale interactions after completion. |
| [`host/tests/reset_handshake.rs`](../crates/host/tests/reset_handshake.rs) | Polling after write acknowledgment, completion, read errors, CPU checks and cancellation. |
| [`scenarios/tests/multi_channel/cases.rs`](../crates/scenarios/tests/multi_channel/cases.rs) | Independently driven hardware/software reset isolation, fresh work and device-wide reset. |
