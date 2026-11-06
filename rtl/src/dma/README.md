# DMA hardware

Own the direct DMA's channel state, shared command execution, register effects
and AXI interfaces. Channels share execution capacity; they are not independent
copies of the engine. Channel state and stage-local state must have explicit owners.

Function ownership, managed access windows and context-table resolution belong
to the deferred sibling `hw_components` directory.

## Implemented register path

`DmaRegisterInterface` assembles `RegisterFrontend` → `common_cell::DuplexMailbox` →
`DmaRegisterBank`. The frontend pairs independent AW/W and retains B/R responses;
the CDC uses the standard library's acknowledged mailboxes. The bank routes
accesses to `ChannelRegisters`, which composes two `RingRegisters`. Each ring
owns its configuration and cursors; the bank has no shadow channel storage.
`RegisterAccess::leaf` computes a candidate access without committing effects.
Only the owning module commits a validated write at the core handshake.
The frontend delegates AW/W/AR storage to `common_cell::ReadyValidSlice` while
retaining AXI credit, pairing and selection policy. The memory crossing also
uses `DuplexMailbox`; there is no parallel DMA-specific CDC implementation.

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
tree. CDC startup waits for both domains. Independently resetting one live domain
is unsupported. Software channel/DMA reset must leave the transport running.
The memory clock domain has no implementation in this slice.

The register modules alone do not implement transaction drain or execution.
`DmaCopy` supplies a first single-channel Linear Copy/Fence32/64 owner and memory endpoint;
see the [slice boundaries](../../../docs/rtl/minimal-copy.md). Its software reset
closes admission and consumes all accepted memory receipts before register clear.
Native tests verify digital handshake/state behavior; bundled-data CDC constraints
and CDC/RDC/timing signoff remain required.
