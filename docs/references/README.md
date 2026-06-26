# PDMA Reference Index

This index records the external schemes, specifications, codebases, and papers that influence PDMA architecture decisions. References here are not requirements by themselves; each white-board decision should still explain the concrete PDMA reason and tradeoff.

## DMA Software Compatibility

- Linux DMA Engine API Guide: https://docs.kernel.org/driver-api/dmaengine/client.html
  - Useful for generic DMA client flow: prepare descriptor, submit, issue pending work, and wait for callback or status.
  - Supports PDMA's decision to expose a baseline Linux DMAengine mapping for common DMA operations.
- Linux DMAengine controller documentation: https://docs.kernel.org/driver-api/dmaengine/provider.html
  - Useful for provider-side capabilities such as `DMA_MEMCPY`, `DMA_INTERLEAVE`, descriptor construction, residue reporting, and out-of-order completion constraints.
  - Supports PDMA's split between generic DMA compatibility and richer subsystem-specific descriptors.
- DPDK DMA Device Library: https://doc.dpdk.org/guides/prog_guide/dmadev.html
  - Useful for dmadev, vchan, enqueue, submit/doorbell, completion, status, and telemetry concepts.
  - Supports PDMA's DPDK compatibility boundary and the choice to keep advanced graph/compute features in a dedicated SDK.
- DPDK `rte_dmadev.h` API reference: https://doc.dpdk.org/api/rte__dmadev_8h.html
  - Useful for exact API names and the dmadev/vchan programming surface.

## Queue, Doorbell, And Completion Patterns

- NVM Express specifications overview: https://nvmexpress.org/specifications/
  - Useful for submission/completion queue identity, doorbell registers, completion records, and host/device queue contracts.
  - PDMA borrows the explicit queue/completion separation but does not inherit NVMe storage-command semantics.
- OASIS Virtio v1.3 specification: https://docs.oasis-open.org/virtio/virtio/v1.3/virtio-v1.3.html
  - Useful for virtqueues, driver/device notification, feature negotiation, and memory-resident descriptor/ring structures.
  - Supports PDMA's rule that doorbells are notifications while memory-resident queue state remains authoritative.
- NVIDIA RDMA Aware Programming User Manual, verb context operations: https://networking-docs.nvidia.com/rdmaawareprogramming/verb-context-operations
  - Useful for queue pairs, completion queues, completion channels, protection domains, and shared completion queue patterns.
  - Supports PDMA's use of queue identity in completions and shared completion ring discussions.
- RDMA core libibverbs man pages: https://github.com/linux-rdma/rdma-core/tree/master/libibverbs/man
  - Useful as open-source reference material for completion queue APIs and poll semantics.

## Accelerator Work Queues And Resource Groups

- Intel Data Streaming Accelerator User Guide: https://cdrdv2-public.intel.com/759709/353216-004-intel-data-streaming-accelerator-user-guide.pdf
  - Useful for work queues, descriptor submission portals, dedicated versus shared work queues, groups, engines, and software-visible queue provisioning.
  - Supports PDMA's queue group/resource-binding vocabulary and acts as a contrast point for device-resident work queues.
- Intel Data Streaming Accelerator Architecture Specification: https://kib.kiev.ua/x86docs/Intel/DSA/341204-003.pdf
  - Useful for architectural details around work queues, portals, groups, engines, and submission feedback.
  - Treat as a reference pattern only; PDMA's preferred scalable direction is memory-resident queue configuration rather than device-resident WQs.
- DPDK IDXD DMA Device Driver: https://doc.dpdk.org/guides/dmadevs/idxd.html
  - Useful for how Intel DSA appears through DPDK, including work queue configuration and dmadev exposure.

## Modeling, Tracing, And Future Scheduler Backend

- `etha_model`: https://github.com/shady831213/etha_model
  - Useful as a local Rust hardware functional modeling reference with tests, FFI/header-generation examples, multi-ring behavior, and Chrome trace JSON output.
  - PDMA borrows the modeling style selectively, while keeping v0 ABI byte layouts abstract.
- `terminus`: https://github.com/shady831213/terminus
  - Useful as a future RISC-V ISA simulator backend if the scheduler coprocessor becomes firmware/ISA-defined.
  - v0 deliberately keeps the scheduler behavioral and does not depend on this backend.
- `terminus_cosim`: https://github.com/shady831213/terminus_cosim
  - Useful as a future co-simulation integration reference if PDMA needs an RTL/ISA scheduler co-sim flow.
- Perfetto external trace format documentation: https://perfetto.dev/docs/getting-started/other-formats
  - Useful for Chrome JSON trace compatibility and practical visualization/analysis constraints.
- Chrome/Catapult trace event format pointer: https://chromium.googlesource.com/catapult/+/refs/heads/main/docs/trace-event-format.md
  - Useful as the historical trace-event format reference; Perfetto documentation is preferred for current tooling behavior.

## Research Leads To Evaluate Later

- Queue scheduling and QoS papers for weighted round-robin, strict-priority control queues, and credit-based arbitration.
- Accelerator task-graph scheduling papers for dependency tables, graph lowering, and abort propagation.
- Performance modeling papers for abstract-time bottleneck attribution and component budget exploration.

These paper references should be added only when a later PDMA decision needs more than the current specs/code patterns explain.
