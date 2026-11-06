# PDMA: A programmable copy engine for virtualized systems

PDMA provides programmable data movement, synchronization and recovery through
multiple channels sharing one Copy Engine. It supports a direct configuration
and a firmware-managed configuration for sharing resources between Functions
and address-space contexts. PCIe endpoints and GPU/SoC integration are target
environments; the engine's memory and control interfaces keep platform mechanisms
outside the data path.

## Architecture and implementation status

| Layer | Responsibility | Current status |
| --- | --- | --- |
| Copy Engine core | Channel arbitration, command execution, completion, fault/resume and independent channel reset | Rust functional model implemented; RTL register-access slice implemented |
| Managed resource control | Firmware channel allocation and lifecycle policy; peripheral MMIO permissions, Function/context resolution and notification routing | Executable managed model implemented; corresponding RTL/subsystem integration deferred |
| Integration platform | Trusted requester identity, memory backing/translation, interconnect and interrupt transport | Injected model capabilities; actual PCIe/IOMMU integration unimplemented |

Function/context isolation and Default/PASID bindings are modeled under the
documented platform and software obligations. This is not a claim of complete
SR-IOV, PCIe FLR, ATS/PRI or logical-channel oversubscription support. Physical
channels remain instance-configured. The direct configuration remains usable
without firmware or virtualization management.

The active Rust model lives in [model/modeling](model/modeling/README.md). It
implements instance-configured channels with command/error-event rings, independent
linear/scatter-gather endpoints and one shared DMA execution pipeline. Copy, Write,
Fill, Wait, Atomic, Semaphore, Fence, Trap and Timestamp share its fault, retirement
and reset boundaries. Successful commands produce no response record: software
observes Fence storage and optional Trap notifications. Recoverable target page
faults retain work for owner resume while peer channels remain eligible.

Hardware behavior is deterministic and step-based. Memory backing, software CPU
execution and interrupt delivery are supplied outside the hardware model.

RTL lives in [rtl](rtl/README.md), with architecture inputs in
[docs/rtl](docs/rtl/architecture.md). The first implemented slice is the DMA
32/64-bit AXI4-Lite register frontend, reg/core CDC and object-owned registers,
covered by native block tests. The execution engine and AXI5 memory port are not
implemented yet; compiler smoke tests remain separate from design verification.
The first RTL phase is DMA only; CPU, peripheral hardware and full subsystem
integration follow after DMA convergence.

## Repository structure

| Directory | Responsibility |
| --- | --- |
| `model/modeling/` | Executable Rust model, firmware, Host software, contract and system specification |
| `rtl/` | Veryl design and native block tests |
| `verification/dma/` | System-level RTL testbench, VIP integration and scenarios |
| `impl/pdma/` | Synthesis, timing constraints and CDC/RDC analysis |
| `tools/rtl-flow/` | Toolchain and shared flow integration |
| `docs/rtl/` | RTL architecture decisions and model-to-hardware mapping |

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
The pre-rebuild implementation is retained at commit
`51707fe1d38e261a188e1d0ac14243c0caf58127`.

## License

PDMA-authored content is dual-licensed under either the [MIT License](LICENSE-MIT)
or the [Apache License, Version 2.0](LICENSE-APACHE), at your option. This covers
the RTL, executable model, firmware, Host software, tools, tests and documentation.
The SPDX license expression is `MIT OR Apache-2.0`.

Third-party material retains its own copyright notices and license terms.
Dependencies and linked specifications are not relicensed by this repository.
Veryl compiler patch origins are recorded in
[the patch provenance document](tools/rtl-flow/patches/README.md).
