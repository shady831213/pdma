# Managed DMA fatal handling and PF events

Status: implemented behavior; verification links below. Firmware
is part of this specification. This chapter extends the control-channel contract;
it does not specify PCIe fatal recovery or firmware crash recovery.

## Fault ownership and containment

**HW-FATAL-1.** A DMA-wide internal fatal latches the existing DMA fatal register,
stops new command acceptance and issuance of further old-work memory accesses.
Already completed writes remain visible. Committed RTL bus transactions may still
complete; a fatal notification does not certify drain or authorize memory reuse.
The model checks before each synchronous memory access. Writes to channel data blocks in the
fatal scope fail; administrative reset, global IRQ enable and status reads remain available. Accepted
work retires through the existing pipeline ownership boundary.

**PLAT-FATAL-1.** In managed mode route the DMA-wide fatal notification to firmware
only. Channel completion/fault notifications remain routed to their channel's
Function. Neither PF nor VF obtains direct access to DMA administration by opening
a channel window. Direct-DMA mode retains its privileged software interface.

**FW-FATAL-1.** Firmware enables DMA fatal reporting and checks sticky fault state
at startup and while serving management work. A fatal closes every data window,
retains channel owners/context bindings, and blocks allocation, channel reset and
context-management requests with a `DeviceFailed` response. Hardware containment
does not wait for firmware scheduling. A PF response-ring-full condition must not
prevent fatal observation and window closure. An accepted lifecycle operation is
not silently cancelled and replayed; its completion must not reopen access after
a fatal. Firmware publishes one fatal event per fault episode.

Firmware checks sticky fault before admission and after accepted operations. Its
accepted reset/maintenance future remains alive across ISR wakes and waits for the
hardware operation's terminal state. A fatal cannot turn a still-pending context
maintenance operation into an acknowledged cleanup. Already committed management
writes are not rolled back: a failure response is not a claim that no side effects
occurred. Existing bindings at the start of DMA recovery remain intact through that
recovery. Window closure consists of separate firmware MMIO writes; hardware fatal
containment protects the interval before all windows have closed, including a
previously issued window-open write that resumes after fatal observation.

**HOST-FATAL-1.** PF handles device-wide recovery and informs affected VFs through
a platform-provided PF/VF mechanism. VF software must not assume that firmware
Ready or a previously granted channel means DMA remains operational. Absence of
an error event is not proof of completion or safe memory reuse; successful work
uses the explicit Fence contract. Cancelled work requires acknowledged cleanup.
Drivers/ISRs must handle rejected data accesses after containment, stop using the
old ring driver, and retain memory until the coordinated reset acknowledgment.
Such access failures must not disable the PF's independent management/event service.

## Event transport

**FW-EVENT-1.** The PF control descriptor contains three independent in-memory
rings: command, response and event. `ControlEvent` initially carries `DmaFatal` and
the DMA fault code, without a request ID. Request completion remains on the
response ring. All three use the shared software ring protocol and PF control
memory attributes; no data context-table slot is reserved for events.

**HOST-EVENT-1.** PF allocates and initializes all three rings before control enable,
owns event consumer and keeps storage valid. Firmware owns event producer. Event
geometry, like command/response geometry, is frozen while attached. PF installs
the event ISR before enabling control. Firmware handles a sticky fatal predating
attachment once event transport is available, without accepting unsafe commands.

**HW-EVENT-1.** Management hardware provides distinct response and event doorbell
registers writable only by firmware; the PF request doorbell remains distinct.
Writing one produces a coalescing notification, zero does nothing. There is no
event payload or software handler in hardware. An event-error register is PF
readable and firmware writable, with `None`, `Overflow`, `AccessError` and
`InvalidRing`. VF cannot access these management registers.

**PLAT-EVENT-1.** Deliver event and response notifications as distinct logical
causes. They may share physical transport only if software can distinguish both.
Publish entry bytes before producer and producer/error state before the doorbell;
consumer must acquire the publication before inspecting bytes. Register/memory
visibility and interrupt ordering are platform guarantees, not Tokio guarantees.

**FW-EVENT-2.** Event full is a delivery failure, not backpressure. On full, memory
access error or invalid event geometry/cursors, retain the first error in the
event-error register, stop event publication and ring the event doorbell even when
no entry was published. Do not overwrite unconsumed entries, queue retries or stop
the control service. Publish producer only after successful entry storage; after
an access error, issue no further publication writes. A failed producer store may
have an uncertain externally visible outcome. The event stream is now
incomplete. Later faults still receive containment but do not replace the first
event error or create a backlog. A partial memory write is not rolled back.

**HOST-EVENT-2.** On event notification, check error before consuming entries. A
nonzero error requires handling incomplete event history, including coordinating
all potentially affected Functions. Normal event consumption is nonblocking;
waiting for an event is independent of waiting for a control response.

PF programs servicing both streams use `try_event` and `try_response`, then wait
on their separate ISR hints when neither can progress. `next_event` and
`next_response` are single-stream convenience waits; no background task drains
either queue. One PF program owns each consumer and retains its wake hints across
checks, avoiding an interval between checking the queue and arming its wait.

## Recovery boundaries

**HOST-RECOVERY-1.** Before `ResetDma`, PF stops submissions and ring/ISR activity
on every affected Function, coordinates VF software and retains all old memory.
PF submits this device-wide operation through its existing management command
ring. It is not a VF request and does not reset the PF control channel.

**FW-RECOVERY-1.** `ResetDma` closes all data windows, requests whole-DMA reset and
awaits actual hardware completion. It restores fatal notification enable and
returns success only after cleanup and a clear fault observation. It preserves
firmware, management rings, event error, channel allocation and context bindings.
Windows stay closed. PF can subsequently use `ResetChannel` for each retained
grant to reopen an unconfigured channel, then its owner programs fresh DMA rings.
A new fatal observed before the final clear-fault check prevents recovery success.
Success certifies that observation, not perpetual health: a later fatal is a new
episode, including one arriving while the success response is being delivered.
Recovery is explicit,
never an automatic retry of failed DMA tasks.

**HW-RECOVERY-1.** Reset completion follows the reset chapter's retirement/drain
contract. DMA reset clears DMA rings/configuration/fault/IRQ state; it does not
clear peripheral ownership, PF control context, firmware state or event errors.

**HOST-RECOVERY-2 / FW-RECOVERY-2.** Event recovery is independent. After observing
an event delivery error (producer stopped), PF repairs storage and reinitializes
the event ring's cursors/geometry while preventing concurrent event consumption,
then submits `ResetEvents`. Firmware validates the new event ring, clears its
error only on success and resumes future event publication. There is no replay of
lost history. Calling this operation while event delivery is healthy is rejected;
it must not race a live producer. Command/response geometry and PF control memory
attributes are unchanged. DMA reset alone cannot repair event delivery.

`ResetDma` and `ResetEvents` are device-wide PF operations; their Function,
channel, context and attributes request fields are ignored. They retain ordinary
request-ID correlation. Event reset can be processed while DMA is failed. A failed
event reattachment returns `InvalidState` and preserves the original error.
Failure of the management register transport itself is a subsystem integration
failure, not an event-memory error that firmware can report through that transport.

## Wire and register interface

Contract schemas are authoritative; these are relative offsets, not platform bases.

| Object | Size/alignment | Definition |
| --- | --- | --- |
| ControlEvent | 8/4 bytes | kind u32 at 0 (`DmaFatal=0`); DMA code u32 at 4 |
| ControlQueue | 72/8 bytes | command state at 0; response at 24; event at 48 |
| ControlRegisters | extent 36, alignment 8 | bootstrap at 0, enable at 8, attributes at 12, request at 20; notification block at 24 |
| ControlNotificationRegisters | extent 12, alignment 4 | child-relative response at 0, event at 4, event error at 8; firmware owns writes, PF reads event error only |
| ControlRequest | unchanged 32/8 bytes | `ResetDma=7`, `ResetEvents=8` |
| ControlResponse | unchanged 24/8 bytes | `DeviceFailed=10`, `InvalidState=11` |

Event memory is initialized before control enable. Event producer and consumer are
u32 wrapping entry counts using the same capacity rule as the other software rings.
Every successful publication rings the event doorbell; notifications can coalesce,
so PF drains available events rather than treating notification count as event count.
Event consumption needs no space-return doorbell because firmware never waits for
event space. The event-error code is bits 0..1; other bits are reserved. Firmware
retains it across subsequent failures until successful `ResetEvents`.

## Model and verification boundary

**MODEL-FATAL-1.** Fault injection uses `Dma::signal_fatal` at a hardware boundary,
not a Host write or a runtime-generated management event. Real firmware reads
registers, changes permissions and publishes events. Runtime only transports
memory, MMIO and interrupts and drives the existing hardware pipeline.

Required acceptance: ordinary direct/managed copies; fatal during active work;
firmware containment with response full; PF-only event delivery; no event retry or
overwrite on full; access-error and corrupt-ring status; successful DMA recovery
and fresh transfer with retained ownership; event-error retention across DMA reset;
explicit event recovery; no old DMA accesses after reset completion. The following
tests cover these model boundaries. Physical outstanding-drain, CDC and release/acquire
properties additionally require integration/RTL verification.

| Requirements | Executable boundary |
| --- | --- |
| HW-FATAL-1, HW-RECOVERY-1 | [DMA fatal containment](../crates/dma/tests/fatal_containment.rs), [reset protocol](../crates/dma/tests/reset_protocol.rs) |
| FW-FATAL-1, response backpressure and retained reset | [Firmware fatal cases](../crates/firmware/tests/control/cases/fatal.rs) |
| FW-FATAL-1, accepted maintenance must terminate before recovery | [Context/fatal boundary](../crates/firmware/tests/control/cases/context_fatal.rs) |
| FW-EVENT-2, malformed geometry/cursors, access failure and no retries | [Firmware event cases](../crates/firmware/tests/control/cases/events.rs) |
| HW-EVENT-1, requester ownership | [Peripheral event access](../crates/hw-components/tests/control/events.rs), [C view acceptance](../crates/contract/tests/header_c/host.c) |
| Independent event consumption and request correlation | [Host event test](../crates/host/tests/management_async/events.rs) |
| PLAT-FATAL-1, PLAT-EVENT-1 logical routing; FW/HOST recovery | [Managed fatal recovery](../crates/scenarios/tests/firmware_managed/harness/runtime/tests/fatal.rs) |

PF/VF coordination uses a platform mechanism outside PDMA. No mailbox or PCIe
signaling is claimed by these tests. C11 fixtures verify generated bytes and view
permissions; they do not establish cache coherency or real bus ordering.
