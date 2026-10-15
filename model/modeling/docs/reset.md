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
This includes parked payload-fault work and its retained read scratch; no repair
or RESUME is required to finish reset. The visible FAULT_STATUS.faulted bit clears.
Software must serialize repair/resume with reset and retire old fault handlers and
their pending requests before reconfiguration or regrant. A late write-one RESUME
can clear a later fault; hardware does not identify which episode it was meant for.
Peer channel state and accepted work remain intact, although cleanup of the shared
engine can delay peers. Channel reset cannot clear a device-wide fault.

Device reset subsumes pending channel resets, blocks new work on every channel and
waits for all affected cleanup. It resets every channel plus device-wide fault and
controls, preserving the instance's read-only `channel_num`.
Platform-specific MMIO ordering and completion must uphold that software lifecycle
exclusion; program order in one issuer is not synchronization between other actors.

Neither scope rolls back external memory writes already performed. Notifications
already handed to the platform remain pending until the platform claims them;
reset does not retract them.

## How the current model reaches completion

`Dma::service()` assembles finite candidate/selection/execution dataflow and
independent IRQ/reset hardware units. External HardwareDriver supplies subsequent
service inputs. Reset checks advance on every top-level tick, including idle or
unfinished command processing; they do not depend on an AfterEach child-step hook.

Channel owns the accepted descriptor ledger, installed traversal and any concrete
read/write fault checkpoint. Fetch advances consumer and installs the ledger in
one synchronous step. Validation, current payload and completion can reside in
typed stage values without releasing accepted ownership. Later stages borrow
only the selected channel and execution resources; they check generation and
reset before each effect. No leaf invokes another stage.

A healthy command retains its hardware grant across finite chunk passes. The
occupied execution unit backpressures command admission; ordinary stages prepare,
read, optionally transform and write one chunk before the next pass. Reset still
advances independently every top-level tick and access guards apply throughout.
Chunk completion is not reset completion or accepted-command retirement.

Payload fault ends the current finite execution path, releases the shared grant
and leaves the concrete hardware task in Channel. Resume derives new work from
that state. A failed write retries from retained transformed scratch without
refetching or replaying Read/Transform. It enters the same configured write tail,
including custom downstream stages. No generic child continuation owns the task.
An early RESUME does not restore the released grant of the old invocation; that
invocation must exit before arbitration starts the resumed one.
Finite service completion is not command retirement while Channel still has work.

Reset cleanup clears affected channel-owned work before acknowledging. A stale
pipeline token cannot issue new effects after generation invalidation, even if its
Rust invocation object remains alive. An arbitrary custom state's destructor is
not a hardware drain predicate. Custom hardware with persistent resources must
represent those resources and their retirement conditions explicitly in hardware
state, rather than hide them in a stage wrapper. A disabled active peer must not
prevent another channel's reset or parked Abort cleanup.

`MemoryAccess` calls are synchronous and finish within a hardware step. MMIO has
exclusive device access, so a reset request occurs between steps, not halfway through
a memory call. Thus the model has no bus transactions, outstanding read/write queues,
cycle timing or artificial drain delay. Cleanup retires channel-owned task state;
the completion guarantee covers every affected memory path.

Channel generation increments once for a new reset request. Repeated requests do
not advance it, and cleanup does not zero it. Work tags retain the generation so
custom stages can reject stale work even after reconfiguration. This
is an additional stale-task guard, not the reset acknowledgment predicate.

Custom finite graphs use `Dma::service_with` and ordinary composition. Their stages
receive concrete chunk, payload or completion data and a selected borrowed
ExecutionContext, not the full Dma. Standard memory and publication stages check
the private generation-bound identity before effects; a transform may modify
owned bytes but cannot replace their geometry or identity. Persistent custom
hardware effects need explicit storage and retirement rules. Merely dropping a
software waiter or pipeline invocation does not request reset or clear the
accepted task. Conversely, retaining obsolete data does not prevent hardware
cleanup once it has no authority to issue further effects.

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
With Function-separated memory, closing the window preserves its memory binding
for old work. Firmware changes the owner only after reset completion; a later
grant uses the new owner's space. Reset generation checks still precede old-task
accesses. No live owner rebind or external transaction drain is introduced.
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
| [`dma/tests/reset_protocol.rs`](../crates/dma/tests/reset_protocol.rs) | Visible busy state, rejected configuration, channel-owned work cleanup, repeated/zero writes and device reset scope. |
| [`dma/tests/multi_channel/reset_boundaries.rs`](../crates/dma/tests/multi_channel/reset_boundaries.rs) | Reset across command/payload/response interactions, preserved peer work and fresh configuration. |
| [`dma/tests/multi_channel/scatter_gather_reset.rs`](../crates/dma/tests/multi_channel/scatter_gather_reset.rs) | SG access boundaries and absence of stale interactions after completion. |
| [`host/tests/reset_handshake.rs`](../crates/host/tests/reset_handshake.rs) | Polling after write acknowledgment, completion, read errors, CPU checks and cancellation. |
| [`scenarios/tests/multi_channel/cases.rs`](../crates/scenarios/tests/multi_channel/cases.rs) | Independently driven hardware/software reset isolation, fresh work and device-wide reset. |
