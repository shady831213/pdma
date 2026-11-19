# DMA hardware

Own the direct DMA's channel state, shared command execution, register effects
and AXI interfaces. Channels share execution capacity; they are not independent
copies of the engine. Channel state and stage-local state must have explicit owners.

Function ownership, managed access windows and context-table resolution belong
to the deferred sibling `hw_components` directory.

The source tree groups hardware responsibilities. Module directories show containment;
packages and interfaces instead declare shared types/protocols within that responsibility.
The register request/response protocol lives in `bus_cell::RegisterPort`. AXI
pairing belongs to the frontend, and DMA register decoding belongs to the control plane.
See the [implemented microarchitecture](../../../docs/rtl/microarchitecture.md)
for the actual instance tree, arbitration owners, state machines and pipeline cuts.
Directory nesting does not add Veryl namespaces, wrappers, registers or latency.

```text
copy.veryl                         DmaCopy: static composition, no private controller
layout.veryl                       Generated contract constants
control/
  control.veryl                    DmaControl: registers, IRQ delivery, reset collection
  interface.veryl                  DmaChannels: typed engine/resource boundary
  registers/
    interface.veryl                RegisterPort → CDC → bank
    access.veryl                   Register access validation
    state.veryl                    Channel/ring configuration, actions and status types
    bank/
      bank.veryl                   Global state and channel address routing
      channel/channel.veryl        ChannelRegisters: channel state and effects
      channel/ring.veryl           RingRegisters: command/event ring state
execution/
  engine.veryl                     CopyEngine: shared arbitration/command execution
  command.veryl                    Accepted command context and phases
  copy/window.veryl                CopyWindow: reserved rows and shared payload RAM
memory/
  interface.veryl                  Typed memory requests/receipts
  crossing.veryl                   DmaMemoryCrossing: request and receipt CDC queues
  axi.veryl                        CopyMemoryEndpoint: DMA identity and error encoding
```

`DmaCopy` connects the `bus_cell` register frontend, control, engine, memory crossing
and memory endpoint. `DmaChannels` carries control/engine wires.
Each channel's registers remain in `control.csr.owners.channels[i].owner` and its
rings in `command_ring`/`event_ring`. Accepted-command state and retained bytes
belong to the execution engine. One shared RAM is banked across channels; there
is still one shared engine, not one engine per channel.

A different engine can connect to the same channel and memory ports. It must
honor admission, cursor-update, error and reset/drain obligations; control does
not inspect its FSM or private buffers. The crossing owns the CDC queues and the shared bus master owns outstanding
credits, so replacing an engine does not require copying that transport. Neither
boundary adds a pipeline register. See [configuration](../../../docs/rtl/configuration.md)
for independent inputs, derived geometry and qualified-profile limits.

The configured simulation/implementation top adds only a static `dma` instance
under `DmaConfigured`. Its native type arguments, VIP settings and implementation
inventory come from the same immutable profile. Hierarchical checkers and design
constraints use this actual instance tree.

## Implemented register path

`DmaRegisterInterface` connects `common_cell::DuplexMailbox` to `DmaRegisterBank`.
`DmaCopy` places the AXI4-Lite frontend upstream of the mailbox. The frontend pairs independent AW/W and retains B/R responses;
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

## Register-to-control boundary

These are the ports of the standalone `DmaRegisterInterface`. `DmaControl`
connects them to the live `DmaChannels` interface and owns notification delivery.

- `commands`, `events`, `configurations` expose the actual ring/channel state to
  execution owners. Atomic capability bits are immutable parameters.
- `command_advance` accepts a descriptor; `event_advance` publishes a completely
  written error event. Each commits one cursor increment. Their `*_advance_ready` outputs report cursor credit and reset
  blocking only; they do not authorize memory access. Execution must also check
  channel enable, lifecycle and geometry. `event_geometry` supports deciding an
  event-configuration failure; enable already validates command geometry.
- `statuses` and global fatal/busy inputs are live owner state, not storage copies.
  `actions` and `global_reset_request` pulse once at successful CSR commit. Execution
  owners must consume these pulses in the core domain; there is no action FIFO.
- `clear_channels` and `global_clear` mean the responsible lifecycle owner has
  completed cleanup. A reset request alone never asserts these or clears state.
  External reset-busy/fatal inputs must block ordinary writes during cleanup.
  IRQ cause is a view of supplied sticky causes and actual event occupancy;
  notification generation and set-versus-ACK priority remain the IRQ owner's work.

The register interface's reg/core cold resets must assert together and release
synchronously to their respective clocks. `DmaCopy` includes the bus reset in the
same assertion group. Reset synchronization belongs to the enclosing reset
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
