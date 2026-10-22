# Reset contract

Channel and device reset separate a software request from hardware completion.
Completion is the memory-lifetime boundary: affected old work can no longer issue
a command read, SG-list read, payload read/write or event/completion write. It is not a
promise that the old command completed successfully.

## Interrupting context maintenance

An affected privileged channel reset or whole-DMA reset may interrupt pending
context Drain/Abort. The maintenance result becomes failure with the affected
channel bitmap; it is never converted to successful drain. Reset does not depend
on reaching the abandoned producer frontier: it blocks old admissions, cancels
unstarted/retained work, and waits for resident work to retire through the normal
reset path. Failing maintenance must not remove that ownership or residency ledger.
Only actual reset completion permits reuse of the affected channel's resources.
Other channels captured by a failed multi-channel Drain still need their own
cleanup; resetting one channel does not certify the whole context as unused.

Firmware accepts ResetChannel/ResetDma while retaining the original maintenance
request and returns their results under separate request IDs. Reset completion is
not conditional on a signal wait becoming satisfied. Reset never fabricates a
successful dependency update and cannot roll back an already committed write or
atomic update. An unrelated channel reset leaves the maintenance request pending.
This recovery rule is a PDMA system contract, not an HSA-defined DrainContext API.

For RTL, cancellation of the maintenance objective does not cancel obligations
already accepted by an external interface: every affected outstanding transaction
must retire before reset acknowledgment. A hung external interface can prevent
reset completion; firmware must not turn a software timeout into reset success.

## Register handshake and scope

The channel and device `Reset` registers have the same read/write protocol:

| Action or observation | Meaning |
| --- | --- |
| Software writes one | Request reset of this scope. |
| Software reads one | Cleanup is pending or in progress. |
| Software writes zero | No effect; software cannot cancel or acknowledge reset. |
| Software repeats one while busy | Coalesce with the pending request. |
| Hardware clears to zero | Affected accepted work has retired and reset state is installed. |

There is no minimum busy duration and software need not observe one before zero.
An idle scope can complete during request handling, after its state is cleared.
A scope with resident pipeline work remains busy until that work has exited and
cleanup is acknowledged. While busy, otherwise valid configuration/control/ring writes
are rejected with `Error::ResetInProgress`, apart from the reset protocol itself;
register reads remain available. Software may configure and enable the scope again
after it observes completion.

Channel reset clears that channel's configuration, ring pointers, fault, IRQ causes
and notification controls. It discards its buffered payload and unpublished error event.
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

`Dma::hardware()` returns one connected data pipeline. An ordinary external
HardwareDriver advances it; runtime does not schedule separate lifecycle stages.
Reset is an out-of-band request to the owning hardware state, not a data item
waiting for downstream readiness. IRQ causes update at their source mutations.

Channel owns the accepted descriptor ledger, installed traversal and any concrete
read/write fault checkpoint. Fetch advances consumer and installs the ledger in
one synchronous step. Validation, current payload and completion can reside in
typed stage values without releasing accepted ownership. Later stages borrow
only the selected channel and execution resources; they check generation and
reset before each effect. No leaf invokes another stage.

A healthy command retains its grant and residency across chunk child completions.
CommandExecution stays InProgress and feeds each next chunk into its child graph;
the occupied execution unit backpressures outer admission. Reset still
does not wait for downstream readiness and access guards apply throughout.
Chunk completion is not reset completion or accepted-command retirement.

Payload fault ends the current finite execution path, releases the shared grant
and leaves the concrete hardware task in Channel. Resume derives new work from
that state. A failed write retries from retained transformed scratch without
refetching or replaying successful Read/Transform work. WriteCopy consumes its
retained payload at its local input, then uses the same configured completion tail.
Upstream empty inputs are bubbles, not reissued payload. No generic child
continuation owns the task and no controller chooses the stage to resume.
An early RESUME does not restore the released grant of the old invocation; that
invocation must exit before arbitration starts the resumed one.
An execution exit caused by parking is not command retirement while Channel retains work.

Fetch records pipeline residency, separately from the accepted command and shared
execution grant. Flow values retain its identity across connecting buffers and
local processing. Parking can release the grant before a tail exits. An error's
owner-local event push can notify software before its tail has exited, but does
not retire accepted ownership or release residency.
Reset first prevents admission and invalidates old work. A resident stage observes
the request through its borrowed context during step, before data-readiness checks
or further effects. It releases the storage it owns and returns a stopped Flow.
Then passes that result through the remaining data path to Retire. Accepted
execution stages have Infallible domain-error ports so they cannot skip retirement
through Complete(Err). Custom stages forward stopped results without further data
effects, after releasing their own local work. FanOut collects completed branch
results and Then delivers that collection directly to Retire. Retire processes
every receipt, performs terminal accepted cleanup and releases residency; an
earlier error does not skip later cleanup. The first error is returned after the
collection is processed, and branch indices do not select owners. Waiting/parked commands retain
accepted ownership unless reset/Abort requires cancellation. No enclosing controller
observes child completion for cleanup.
Buffered values that have not yet been accepted reach their receiving stage, which
checks the same lifecycle state before acting. Context projection never reaches into
child storage or invokes an external flush method. In RTL, reset similarly must
reach occupied stages independently of data readiness and completion must wait for
their cleanup acknowledgements and any outstanding transactions.

If no pipeline state resides in the scope, its owner can clear retained channel
state and acknowledge immediately. An unrelated stalled channel therefore cannot
delay an idle channel's reset. Device completion waits for all affected scopes.
A stale externally retained data token cannot issue effects after generation
invalidation. Custom hardware with additional persistent resources must represent
their retirement conditions explicitly; dropping arbitrary Rust objects is not
a substitute for draining external bus transactions.

`MemoryAccess` calls are synchronous and finish within a hardware step. MMIO has
exclusive device access, so a reset request occurs between steps, not halfway through
a memory call. Thus the model has no bus transactions, outstanding read/write queues,
cycle timing or artificial drain delay. Cleanup retires channel-owned and pipeline-resident task state;
the completion guarantee covers every affected memory path.

Channel generation increments once for a new reset request. Repeated requests do
not advance it, and cleanup does not zero it. Work tags retain the generation so
custom stages can reject stale work even after reconfiguration. This
is an additional stale-task guard, not the reset acknowledgment predicate.

Custom finite graphs use `Dma::hardware_with` and ordinary composition. Their stages
receive concrete chunk, payload or completion data and a selected borrowed
ExecutionContext resource bank, not the full Dma. Each Flow identifies its owner;
custom stages observe Flow::lifecycle(context), and standard memory stages and owner-local reporting check
the private generation-bound identity before effects; a transform may modify
owned bytes but cannot replace their geometry or identity. Persistent custom
hardware effects need explicit storage and retirement rules. Merely dropping a
software waiter does not terminate the stage, request reset or clear the
accepted task. Externally retained obsolete data does not prevent hardware cleanup
once it has no authority to issue effects, but retained Flow is an owned pipeline
transfer, not an external non-owning observation. It must reach Retire before reset
can complete. Custom stages observe lifecycle before readiness checks and effects,
release their own transient state, and output the stopped Flow. A custom stage
that ignores reset cannot be made quiescent by composition; reset remains busy
until that work exits and reaches retirement. A stale unaccepted command observation
does not hold residency and cannot substitute a newer command after reconfiguration.

## Software waiting and memory lifetime

Direct-DMA Host `Channel::reset(self)` and `Channel::reset_dma(self)` consume the driver,
write the request and repeatedly read the same reset register until zero. The first
read may already return zero. The transport must order that read after acceptance
of the request write; reading an old zero before the request arrives is not a
completion observation. Reset waits
have no driver-imposed CPU affinity; the supplied bus enforces any resource-specific
access restriction. Busy polls cooperatively yield so the
independently driven hardware can progress. A successful return means hardware
completion was observed. Reuse requires fresh channel setup.

Caller deadlines and cancellation are software policy. Dropping the wait does not
cancel an accepted hardware request, and a timeout or failed poll does not establish
completion. Keep affected command rings, SG lists, payload buffers and event/completion
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
| [`dma/tests/multi_channel/reset_boundaries.rs`](../crates/dma/tests/multi_channel/reset_boundaries.rs) | Reset across command/payload/event interactions, preserved peer work and fresh configuration. |
| [`dma/tests/multi_channel/scatter_gather_reset.rs`](../crates/dma/tests/multi_channel/scatter_gather_reset.rs) | SG access boundaries and absence of stale interactions after completion. |
| [`host/tests/reset_handshake/main.rs`](../crates/host/tests/reset_handshake/main.rs) | Polling after write acknowledgment, completion, read errors, CPU checks and cancellation. |
| [`scenarios/tests/multi_channel/cases.rs`](../crates/scenarios/tests/multi_channel/cases.rs) | Independently driven hardware/software reset isolation, fresh work and device-wide reset. |
