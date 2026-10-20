# Signal and dependency Wait contract

Status: implemented model scope, 2026-10-20. This chapter specifies PDMA commands
and shared software helpers, with explicit platform and RTL obligations. It does
not claim HSA runtime ABI, AQL packet compatibility or whole-platform conformance.

## Representation and operation matrix

**HW-SIG-01.** A signal is one naturally eight-byte-aligned 64-bit word at
`SignalTarget { address, context }`. Values and comparisons are logically signed
`i64`; wire operands and results contain their little-endian two's-complement
`u64` bits. There is no signal width selector. Addition and subtraction wrap
modulo 2^64; bitwise operations affect all 64 bits.

| Command operation | Legal order | Successful observation |
| --- | --- | --- |
| Signal Load | Relaxed, Acquire | Loaded value |
| Signal Store | Relaxed, Release | Value before replacement |
| Signal Add, Subtract, And, Or, Xor, Exchange | Relaxed, Acquire, Release, AcquireRelease | Value before update |
| Signal CompareExchange | Relaxed, Acquire, Release, AcquireRelease | Value before the attempt, whether or not it matches |
| Wait Equal, NotEqual, LessThan, GreaterOrEqual | Relaxed, Acquire | Value satisfying the signed comparison |

CompareExchange replaces the word only when all bits match `expected`. A failed
comparison performs no write; its read has Acquire semantics for Acquire and
AcquireRelease, otherwise Relaxed. Wait compares the observed signed value against
`compare`. GreaterOrEqual is ordinary signed comparison, never cyclic counter
comparison. There is no timeout, wait hint or spurious successful Wait completion.

**HW-SIG-02.** Command is 112 bytes, eight-byte aligned: `id`, `flags`, `opcode`
and tagged payload begin at offsets 0, 8, 12 and 16. Opcode 0 selects Copy, 1
Signal, 2 Wait. Copy's endpoint/range fields live in `CopyCommand`; its behavior
is retained. Response remains 24 bytes with id/status/flags/info at 0/8/12/16.
Copy statuses 0..5 retain their meanings; retired status 6 is invalid.
`SignalSuccess = 7` reports the observed signal bits for Signal or Wait.
`SignalError = 8` reports a reason and stops the channel after publication.
The generated [headers](../include/README.md) and
[contract schemas](../crates/contract/src/dma/command.rs) define the exact layout.

## Execution, progress and errors

**HW-SIG-03.** Same-channel commands execute in submission order. An unsatisfied
Wait retains its accepted command and target reference, blocks successors, releases
the shared execution grant and is reconsidered by arbitration. Other eligible
channels can execute, including the producer needed to satisfy that Wait. The
descriptor is not fetched again. This normal dependency state sets no fault bit
and emits no response, DMA IRQ or PF event; it requires no RESUME request.

**HW-SIG-04.** Each Signal access or Wait observation uses one indivisible atomic
backend operation. A SignalError response reports reason `Unsupported` for unavailable
atomic capability, `Access` for terminal access failure, or `InvalidOperation` for
invalid alignment or an illegal operation/order combination.
These failures commit no update. Invalid wire enum encodings remain descriptor
decode failures under the existing command-fetch fatal policy, not an alternative
way to obtain a SignalError response.

An atomic `PageFault` also commits no update, but is recoverable: it parks the
accepted command, sets FAULT_STATUS and releases the shared grant without a DMA
response/IRQ/event. The platform reports the fault; owner software repairs backing
and requests RESUME through its channel work MMIO. A Wait load can fault too.
The [page-fault repair and source-tag lifetime rules](page-fault-recovery.md) apply.
An already committed atomic update is never reclassified as an uncommitted fault.

**HW-SIG-05.** Hardware retains a successful atomic result as a completion checkpoint
before entering response publication. Response backpressure or publication failure
must not execute the atomic operation again. Normal response/IOC rules apply.
A response write failure retains the existing channel fatal behavior; software
cannot infer from a missing response that the signal update did not happen.
Reset or cancellation never rolls back a committed update.

## Lifetimes and management recovery

**HOST-SIG-01.** Software allocates and initializes signal storage, establishes
authorized mappings, and retains the target and ring contexts while work is queued,
waiting, fault-parked, executing or awaiting response publication. A successful
atomic memory access alone is not a reclamation boundary. Use terminal retirement
or acknowledged cancellation cleanup, under the
[context obligations](context-contract.md), before freeing or rebinding resources.
Direct software signal accesses must also finish before their storage is reused.

**HW-SIG-06.** Drain includes Signal/Wait target references and waits for an ordinary
false Wait to finish normally. It does not report Busy or ChannelStopped merely
because the comparison is false. Page-fault-stopped/error work retains the existing
maintenance failure and failed-channel bitmap behavior. Abort discards matching
unstarted, ordinarily waiting or fault-parked work without publishing a synthetic
success. A running atomic access is indivisible; Abort cannot split or undo it.
Retained accepted references remain until actual cleanup, including resident
pipeline state, is complete.

**HOST-SIG-02.** Software must keep dependency producers and response consumers
progressing during Drain. Hardware cannot diagnose a dependency that will never
be satisfied. A failed request, deadline or dropped waiter is not permission to
reuse signal memory.

**FW-SIG-01.** Existing firmware control requests provide recovery during a pending
Drain: firmware retains that request while accepting ResetChannel/ResetDma.
An affected reset fails the Drain and independently waits for real reset cleanup;
an unrelated reset does not cancel it. Reset can cancel a false Wait without
waiting for the signal or requesting RESUME. No CancelDrain or new firmware signal
service is added. See the [reset contract](reset.md#interrupting-context-maintenance).

## Software, runtime and platform boundaries

**MODEL-SIG-01.** `software::signal::Signal` borrows a supplied unit-sideband
`MemoryAccess` capability and address. It allocates nothing and owns no CPU,
interrupt registration or channel policy. Typed read/write/update orders exclude
invalid software order combinations. Loads return signed values; store returns
unit; update methods return the observed old value. `try_wait` returns an optional
satisfying value, and async `wait` yields on unmet conditions. Dropping that future
stops polling without altering the word. Backend-specific `AtomicAccessError`
details remain intact; no separate ordinary read/write fallback exists.

**MODEL-SIG-02.** DMA interprets the PDMA operation. Generic runtime supplies atomic
word access, and peripheral adapters resolve channel/context authority to the
injected Function/address-space capability. Runtime gains no signal allocation,
context ownership, firmware scheduling or requester policy.

**PLAT-SIG-01.** A backend must serialize an entire atomic access against overlapping
ordinary and atomic accesses to the same storage. Errors precede modification.
Release publishes preceding accesses before its signal update is observable;
Acquire that observes that publication makes the associated preceding accesses
visible to subsequent consumers. Non-relaxed operations share a synchronization
order within the backend's supported domain. Adapters must preserve that domain
across all aliases and access paths used by the cooperating agents.
Local storage executes synchronously; shared storage serializes access in its own
storage domain. Separate backend instances do not automatically provide a common
order or coherence even when they alias the same external memory. Such integration
must establish shared atomicity and visibility explicitly before using these
signals to synchronize across those instances.

A representative dependency chain is producer payload writes, release Signal,
acquire Wait, Copy, release completion Signal, then consumer acquire and payload
reads. PDMA queue order and response publication do not by themselves flush CPU or
device caches, invalidate TLBs, or extend a backend's coherence domain.

**RTL-SIG-01.** Integration must implement these visibility points with actual
coherence, cache maintenance and interconnect ordering where needed, and retain
atomicity across physical request paths. A host-language fence alone is not proof
of device visibility. RTL must track all affected external transactions and
resident work through reset/maintenance completion. The synchronous model contains
no cache, cycle timing, transaction queue or MMU proof.

## HSA reference and deliberate differences

HSA Runtime 1.2 specifies signed signal values, the atomic operation family and
EQ/NE/LT/GTE conditions; its large-machine value is 64 bits. Its wait API may return
an unsatisfying observation after interruption or timeout, and some updates return
void. PDMA instead returns old values for its update helpers and only a satisfying
value from Wait. PDMA's address/context target is not an opaque HSA signal handle.
See [HSA Runtime 1.2, sections 2.4 and 2.4.1](https://hsafoundation.com/wp-content/uploads/2021/02/HSA-Runtime-1.2.pdf#page=44).

HSA defines scoped synchronization and memory-consistency requirements beyond
matching operation names. The modeled backend ordering above is an explicit PDMA
contract; it is not a claim that arbitrary platforms satisfy HSA's scoped model.
See [HSA System Architecture 1.2, chapter 3](https://hsafoundation.com/wp-content/uploads/2021/02/HSA-SysArch-1.2.pdf#page=59).

AQL packets, doorbell signals, HSA runtime integration, logical-channel
oversubscription, general DMA memory-atomic opcodes and a standalone Fence opcode
remain outside this step. Completion signaling uses an explicit Signal command
in queue order; Copy does not gain an implicit completion-signal field.
The Signal stage composes through the existing pipeline framework; this step adds
no generic pipeline combinator or runtime scheduling policy.

## Verification surfaces

These are required behavioral surfaces, not a statement that a particular run passed:

- [Contract tests](../crates/contract/tests) check tagged wire layouts, typed orders,
  signed conditions and retired status rejection.
- [Software Signal tests](../crates/software/tests/signal) exercise all operations,
  signed boundaries, competing updates, unsupported access, visibility and cancellation.
- [DMA Signal tests](../crates/dma/tests/signal) exercise execution and lifecycle;
  [managed scenarios](../crates/scenarios/tests/firmware_managed/signal.rs) cover
  resolved context/Function backing and existing firmware recovery.
- RTL verification must separately prove external atomicity, visibility, fault
  no-commit behavior, context lifetime and transaction retirement across cancellation.
