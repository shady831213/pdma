# Programmable DMA Subsystem (PDMA)

PDMA is an early-stage memory-to-memory programmable DMA subsystem concept. The current notes are exploratory and live under `docs/white_boards` rather than in this top-level README.

## Documents

- [Initial ideas](docs/white_boards/initial_ideas.md): early architecture notes, including software/hardware partitioning, memory layout, queue model, control/data path interaction, optional compute pipeline, and subsystem deliverables.
- [Conceptual model v0 proposal](docs/white_boards/conceptual_model_v0.md): Rust-based modeling proposal for discussing component partitioning, interfaces, async versus step-based execution, tracing, and first scenario tests.

## Scope Snapshot

- Memory-to-memory DMA subsystem framework.
- Optional in-path compute between source and destination memory.
- Scheduler coprocessor for complex task orchestration.
- Linux DMAengine and DPDK DMAdev compatibility for baseline DMA operations.
- Advanced SDK for subsystem-specific capabilities.
- External IOMMU handles isolation policy.
