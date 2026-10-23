# Programmable DMA Subsystem (PDMA)

The active Rust model lives in [model/modeling](model/modeling/README.md). It
implements instance-configured channels with command/error-event rings, independent
linear/scatter-gather endpoints and one shared DMA execution pipeline. Copy, Write,
Fill, Wait, Atomic, Semaphore, Fence, Trap and Timestamp share its fault, retirement
and reset boundaries. Successful commands produce no response record: software
observes Fence storage and optional Trap notifications. Recoverable target page
faults retain work for owner resume while peer channels remain eligible.

Hardware behavior is deterministic and step-based. Memory backing, software CPU
execution and interrupt delivery are supplied outside the hardware model.

## Current documentation

- [Model architecture and verification](model/modeling/README.md).
- [Reset contract](model/modeling/docs/reset.md): current completion and memory-lifetime guarantees.
- [Generated ABI header](model/modeling/include/README.md): generation and consistency checks.
- [System scenarios](model/modeling/crates/scenarios/README.md).
- [System specification](model/modeling/docs/spec.md): hardware, firmware, software and platform obligations, with the model-to-RTL boundary.
- [Roadmap and decisions](model/modeling/docs/firmware-dma-goal.md): implemented firmware-managed channels, Function/context isolation and copy-engine commands, plus deferred integration/recovery directions. Actual PCIe, IOMMU, FLR and logical-channel oversubscription remain outside the current implementation; ordinary DMA still runs without firmware.
- [First milestone: architecture MVP](model/modeling/docs/firmware-dma-mvp.md): implemented firmware-opened Host window and IRQ-driven DMA copy, with separate hardware, firmware and runtime responsibilities. Broader PCIe features remain future work.

## Historical background and references

- [Initial ideas](docs/white_boards/initial_ideas.md) and [conceptual model v0](docs/white_boards/conceptual_model_v0.md) are historical exploration, not current requirements or implementation claims.
- [Reference index](docs/references/README.md) collects research sources, not compatibility commitments.

The pre-rebuild architecture specification and v0.x plans were removed from the
working tree because they described obsolete interfaces. Their last retained
versions are available in Git at commit
`7f264595971aad5224d98276113a11efc63ee2db`, under `model/docs/`.
The pre-rebuild implementation remains on `backup/pdma-before-rebuild-20260909`.
