# DMA hardware

Own the direct DMA's channel state, shared command execution, register effects
and AXI interfaces. Channels share execution capacity; they are not independent
copies of the engine. Channel state and stage-local state must have explicit owners.

Function ownership, managed access windows and context-table resolution belong
to the deferred sibling `hw_components` directory.

The source tree groups hardware responsibilities. Module directories show containment;
packages and interfaces instead declare shared types/protocols within that responsibility.
In particular, `registers/bus.veryl` is the internal register transaction protocol,
not the system bus or a bus-arbitration module. See the [implemented microarchitecture](../../../docs/rtl/microarchitecture.md)
for the actual instance tree, arbitration owners, state machines and pipeline cuts.
Directory nesting does not add Veryl namespaces, wrappers, registers or latency.

```text
copy.veryl                         DmaCopy: clocks, transport, IRQ/reset collection
layout.veryl                       Generated contract constants
registers/
  interface.veryl                  DmaRegisterInterface: frontend → CDC → bank
  frontend.veryl                   AXI4-Lite pairing/response owner
  bus.veryl, access.veryl           Register protocol and access validation
  state.veryl                      Live register-owner interfaces
  bank/
    bank.veryl                     DmaRegisterBank: global state and channel routing
    channel/
      channel.veryl                ChannelRegisters: channel state and effects
      ring.veryl                   RingRegisters: command/event ring state
execution/
  engine.veryl                     CopyEngine: shared arbitration/command execution
  command.veryl                    Accepted command context and phases
  copy/window.veryl                CopyWindow: per-channel burst rows and RAM owner
memory/
  interface.veryl                  CopyMemory request/receipt types and port
  endpoint/
    endpoint.veryl                 CopyMemoryEndpoint: routing and return merge
    read.veryl                     CopyReadEndpoint: AXI read lane
    write.veryl                    CopyWriteEndpoint: AXI write lane
```

The actual top-level instance paths are `csr` (frontend, crossing, owners),
`engine` (copy window), `request_crossing`, `response_crossing`, and `endpoint`
(reader, writer). Register channels are `csr.owners.channels[*].owner`; their two
ring owners are `command_ring` and `event_ring`. Shared execution does not
instantiate one engine per channel. Module and instance names survive file moves,
so constraints, checkers and integration bindings keep their hardware identities.

## Implemented register path

`DmaRegisterInterface` assembles `RegisterFrontend` → `common_cell::DuplexMailbox` →
`DmaRegisterBank`. The frontend pairs independent AW/W and retains B/R responses;
the CDC uses the standard library's acknowledged mailboxes. The bank routes
accesses to `ChannelRegisters`, which composes two `RingRegisters`. Each ring
owns its configuration and cursors; the bank has no shadow channel storage.
`RegisterAccess::leaf` computes a candidate access without committing effects.
Only the owning module commits a validated write at the core handshake.
The frontend delegates AW/W/AR storage to `common_cell::ReadyValidSlice` while
retaining AXI credit, pairing and selection policy. The sustained memory crossing
uses `common_cell::AsyncQueue`, a standard Gray-FIFO wrapper; CDC protocols remain
in the reusable cells rather than DMA-specific synchronization logic.

`layout.veryl` is generated from contract metadata. It contains offsets,
permissions, field masks/positions, enum values and command/event record geometry.
The application selects `DmaRegisters`, `Command` and `Event`; the generic schema
Veryl backend traverses dependencies and emits bank geometry from the map
definitions. No DMA address formula or Veryl renderer remains in the application.
Tests cross-check the admin-page boundary. Generated geometry functions assume
valid instance parameters; they are not runtime software configuration validators.

Supported integration parameters are a 32-bit register address, 32/64-bit AXI4-Lite
data, positive `CHANNEL_COUNT` and a page-aligned `BASE_ADDR` whose complete map
fits the 32-bit aperture. These are elaboration/integration obligations, not
software-programmable settings. The interface does not implement a wider-address
decoder. The register access matrix is specified in
[register-interface.md](../../../docs/rtl/register-interface.md).

## Core owner boundaries

- `commands`, `events`, `configurations` expose the actual ring/channel state to
  the future execution owners. Atomic capability bits are immutable parameters.
- `command_advance` and `event_advance` commit one hardware cursor increment after
  successful work. Their `*_advance_ready` outputs report cursor credit and reset
  blocking only; they do not authorize memory access. Execution must also check
  channel enable, lifecycle and geometry. `event_geometry` supports deciding an
  event-configuration failure; enable already validates command geometry.
- `statuses` and global fatal/busy inputs are live owner state, not storage copies.
  `actions` and `global_reset_request` pulse once at successful CSR commit. Future
  owners must consume these pulses in the core domain; there is no action FIFO.
- `clear_channels` and `global_clear` mean the responsible lifecycle owner has
  completed cleanup. A reset request alone never asserts these or clears state.
  External reset-busy/fatal inputs must block ordinary writes during cleanup.
  IRQ cause is a view of supplied sticky causes and actual event occupancy;
  notification generation and set-versus-ACK priority remain the IRQ owner's work.

Both physical reset inputs must assert together and release synchronously to
their respective clocks; reset synchronization belongs to the enclosing reset
tree. A released source may buffer before its peer starts. Independently resetting one live domain
is unsupported. Software channel/DMA reset must leave the transport running.
The memory endpoint has independent read/write lanes and ordered return tracking.

The register modules alone do not implement transaction drain or execution.
`DmaCopy` supplies one shared Linear Copy/Fence32/64 engine for 1..8 channels and
a shared memory endpoint;
see the [slice boundaries](../../../docs/rtl/minimal-copy.md). Its software reset
closes the target channel's admission and consumes its accepted memory receipts
before register clear. Global reset collects all channels' cleanup. See
[multichannel ownership](../../../docs/rtl/multichannel.md).
Native tests verify digital handshake/state behavior; bundled-data CDC constraints
and CDC/RDC/timing signoff remain required.
