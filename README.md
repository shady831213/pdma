# Programmable DMA Subsystem (PDMA)

The active Rust model lives in [model/modeling](model/modeling/README.md). It
implements instance-configured channels with command/response rings, independent
linear/scatter-gather endpoints, one shared DMA execution pipeline, channel/device
fault and reset handling, and empty-to-non-empty/IOC notifications.

Hardware behavior is deterministic and step-based. Memory backing, software CPU
execution and interrupt delivery are supplied outside the hardware model.

## Current documentation

- [Model architecture and verification](model/modeling/README.md).
- [Reset contract](model/modeling/docs/reset.md): current completion and memory-lifetime guarantees.
- [Generated ABI header](model/modeling/include/README.md): generation and consistency checks.
- [System scenarios](model/modeling/crates/scenarios/README.md).
- [Next goal: firmware-managed DMA](model/modeling/docs/firmware-dma-goal.md): firmware plus DMA as the primary direction for preparing SR-IOV, PASID and Function reset support, without requiring firmware for ordinary configurations. This is not implemented functionality or a request to begin PCIe integration.
- [First milestone: architecture MVP](model/modeling/docs/firmware-dma-mvp.md): validate separate hardware, firmware and runtime responsibilities with a firmware-opened Host window and one real DMA copy. Approved scope, not implemented yet.

## Historical background and references

- [Initial ideas](docs/white_boards/initial_ideas.md) and [conceptual model v0](docs/white_boards/conceptual_model_v0.md) are historical exploration, not current requirements or implementation claims.
- [Reference index](docs/references/README.md) collects research sources, not compatibility commitments.

The pre-rebuild architecture specification and v0.x plans were removed from the
working tree because they described obsolete interfaces. Their last retained
versions are available in Git at commit
`7f264595971aad5224d98276113a11efc63ee2db`, under `model/docs/`.
The pre-rebuild implementation remains on `backup/pdma-before-rebuild-20260909`.
