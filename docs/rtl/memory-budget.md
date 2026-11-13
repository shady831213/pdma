# RAM boundaries during implementation exploration

The DMA functional RTL continues to use the generic RAM provider. Simulation
checks its clocked read/write behavior. Synthesis supports two explicit modes:

- `control-budget` (default): replace inventoried technology-provider instances
  with opaque leaves backed by generated, capacitance-only Liberty cells.
- `generic`: synthesize the same behavioral arrays into standard-cell logic.

The mechanism follows the public
[pcie_modeling implementation flow](https://github.com/shady831213/pcie_modeling/tree/master/impl/ats_port/flow).
It imports no technology library, installation setting or qualification result.

## Ownership and current inventory

`impl/dma/profiles/copy.tcl` owns the selected instances, provider contracts,
geometry and clock domains. Shared serialization, binding and audits live in
`tools/rtl-flow/implementation`. RAM behavior and ports remain with the native
Veryl providers; there is no replacement simulation model or generated RTL stub.

The current inventory is the Copy window's `engine/copy/buffers/u_ram`:
`8 * CHANNEL_COUNT` rows of 512 bits, one synchronous read and one write port on
`core_clk`, with bit write masks and no content reset. The logical reservation is
512B per channel: 1KiB at the development count of 2 and 4KiB at the maximum of 8.
The previous single-channel qualification remains a 512B checkpoint; it does not
qualify the larger configurations.
Same-address collisions retain the common-cell contract; the budget model does
not add forwarding or qualify a physical macro's collision behavior.

The standard-library FIFO arrays, synchronizers, queue control, command state
and registers remain synthesized logic. They are not implicitly blackboxed.
Replacing FIFO storage with an SRAM requires a separate port/latency/clock-domain
design; it is not part of this flow alignment.

## What the generated library means

Each role has its own library reference containing its port contract and geometry.
Equal-shaped RAMs retain distinct identities. Before binding, the flow checks the
exact native instance, parameterized provider and every pin/direction. After
optimization, it locates the leaf by bound library identity, independently of
whether its enclosing RTL hierarchy still exists.

The Liberty supplies pins and input capacitance, but no function or timing arcs.
The zero area field is a serialization placeholder: **physical SRAM area and
power are unknown and excluded**, not zero-cost resources. Non-clock pins get
zero external-delay accounting origins on their declared clock. The reports
measure surrounding logic paths and the budget available to a future macro;
they do not assert zero SRAM tCO, setup or hold time.

Audits require exactly the declared opaque leaves, dimensions, pins, clocks,
native library loads and all min/max rise/fall delay components. Dynamic pins
must have finite timing paths; constant input pins are recorded separately.
Missing/extra leaves, unresolved control hierarchy and lost/changed constraints
fail. Only the inventory-approved RAM library cells are exempt from the standard
cell allowlist; ULVT remains forbidden.

Genus classifies the clock of an arc-free leaf as a data endpoint. The timing
coverage gate admits that one diagnostic only for the exact, inventory-checked
RAM clock pins. It does not accept other endpoints or a matching count alone.
This is not a CDC/RDC waiver or a physical memory timing model.

## Hierarchy optimization and CDC

Ordinary RTL uses `auto_ungroup=both` and `boundary_opto=true`; retiming stays off.
RAM leaves have their own preservation policy. Synchronizer cells remain
protected after mapping, with their exact expected shapes and local paths checked.

Ungroup can replace hierarchy separators in mapped instance names. Genus's native
RTL-origin names therefore supplement current names when resolving package-owned
CDC scopes. The package accepts an optional name resolver; the Genus adapter owns
the tool-specific query. Fresh object collections are resolved after mapping.
No old collection is reused, no name is guessed by replacing underscores, and
no module hierarchy is retained solely to make an audit pass.

The existing bounded crossing delays, held-payload coverage and normal local
synchronizer timing remain required. Structural CDC/RDC still inspect behavioral
RTL with explicit source-bound review; synthesis's RAM abstraction does not hide
storage from those checks.

## Running and interpreting results

```sh
PDMA_MEMORY_MODE=control-budget bash impl/dma/flow/genus.sh "$snapshot" "$new_run"
PDMA_MEMORY_MODE=generic bash impl/dma/flow/genus.sh "$snapshot" "$new_generic_run"
bash impl/dma/tests/genus/memory.sh "$snapshot" "$new_memory_test"
bash impl/dma/tests/genus/memory.sh "$snapshot" "$new_minimum_test" minimum
```

`memory-inventory.tsv` records excluded capacity and bound identities;
`memory-load-{pre,post}.tsv` records actual electrical loads;
`memory-budgets.tsv` and `memory-static-pins.tsv` distinguish timed and constant
pins. `memory-qualification.txt` records mode and interpretation. These raw
artifacts remain private alongside the mapped database and timing reports.

A smaller mapped area after blackboxing is not an equivalent area improvement:
the storage implementation has left the measured boundary. Compare matching
memory modes, source, configuration and optimization policy, and always report
the excluded capacity. Real SRAM integration, hold checks, routed timing, MTBF
and workload power remain separate qualification work.
