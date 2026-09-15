# PDMA Reference Index

This index retains research sources from the original exploration. Inclusion does
not establish current implementation, compatibility or future scope. See the
[active model](../../model/modeling/README.md) and
[next goal](../../model/modeling/docs/firmware-dma-goal.md) for current boundaries.
Source URLs are retained here; this documentation cleanup does not revalidate
external links or claim that the linked editions are the latest.

## DMA Software Compatibility

- Linux DMA Engine API Guide: https://docs.kernel.org/driver-api/dmaengine/client.html
  - Useful for generic DMA client flow: prepare descriptor, submit, issue pending work, and wait for callback or status.
  - A possible future software integration reference; no Linux DMAengine compatibility is claimed by the current model.
- Linux DMAengine controller documentation: https://docs.kernel.org/driver-api/dmaengine/provider.html
  - Useful for provider-side capabilities such as `DMA_MEMCPY`, `DMA_INTERLEAVE`, descriptor construction, residue reporting, and out-of-order completion constraints.
  - Background for evaluating generic DMA APIs against device-specific descriptors.
- DPDK DMA Device Library: https://doc.dpdk.org/guides/prog_guide/dmadev.html
  - Useful for dmadev, vchan, enqueue, submit/doorbell, completion, status, and telemetry concepts.
  - A possible future software integration reference; DPDK compatibility and a graph/compute SDK are not current model features.
- DPDK `rte_dmadev.h` API reference: https://doc.dpdk.org/api/rte__dmadev_8h.html
  - Useful for exact API names and the dmadev/vchan programming surface.

## Queue, Doorbell, And Completion Patterns

- NVM Express specifications overview: https://nvmexpress.org/specifications/
  - Useful for submission/completion queue identity, doorbell registers, completion records, and host/device queue contracts.
  - Reference for queue/completion separation, not a requirement to inherit NVMe storage-command semantics.
- OASIS Virtio v1.3 specification: https://docs.oasis-open.org/virtio/virtio/v1.3/virtio-v1.3.html
  - Useful for virtqueues, driver/device notification, feature negotiation, and memory-resident descriptor/ring structures.
  - Compare queue state and publication rules with PDMA's own contract; do not import virtio ownership rules implicitly.
- NVIDIA RDMA Aware Programming User Manual, verb context operations: https://networking-docs.nvidia.com/rdmaawareprogramming/verb-context-operations
  - Useful for queue pairs, completion queues, completion channels, protection domains, and shared completion queue patterns.
  - Historical background for shared completion ring discussions, not a current shared-ring requirement.
- RDMA core libibverbs man pages: https://github.com/linux-rdma/rdma-core/tree/master/libibverbs/man
  - Useful as open-source reference material for completion queue APIs and poll semantics.

## Accelerator Work Queues And Resource Groups

- Intel Data Streaming Accelerator User Guide: https://cdrdv2-public.intel.com/759709/353216-004-intel-data-streaming-accelerator-user-guide.pdf
  - Useful for work queues, descriptor submission portals, dedicated versus shared work queues, groups, engines, and software-visible queue provisioning.
  - Background for resource partitioning and comparison with device-resident work queues.
- Intel Data Streaming Accelerator Architecture Specification: https://kib.kiev.ua/x86docs/Intel/DSA/341204-003.pdf
  - Useful for architectural details around work queues, portals, groups, engines, and submission feedback.
  - Reference patterns only; this index does not select PDMA's queue-configuration storage or require DSA-style work queues.
- DPDK IDXD DMA Device Driver: https://doc.dpdk.org/guides/dmadevs/idxd.html
  - Useful for how Intel DSA appears through DPDK, including work queue configuration and dmadev exposure.

## Modeling, Tracing, And Possible Firmware Backends

- `etha_model`: https://github.com/shady831213/etha_model
  - Useful as a local Rust hardware functional modeling reference with tests, FFI/header-generation examples, multi-ring behavior, and Chrome trace JSON output.
  - Reference for composition patterns; PDMA's current byte layouts come from its own contract schemas and generated header.
- `terminus`: https://github.com/shady831213/terminus
  - A candidate RISC-V ISA simulator if future firmware modeling needs instruction execution; no backend choice has been made.
- `terminus_cosim`: https://github.com/shady831213/terminus_cosim
  - A possible reference if later firmware modeling needs RTL/ISA co-simulation; not a current dependency or requirement.
- Perfetto external trace format documentation: https://perfetto.dev/docs/getting-started/other-formats
  - Useful for Chrome JSON trace compatibility and practical visualization/analysis constraints.
- Chrome/Catapult trace event format pointer: https://chromium.googlesource.com/catapult/+/refs/heads/main/docs/trace-event-format.md
  - Useful as the historical trace-event format reference; Perfetto documentation is preferred for current tooling behavior.

## Research Leads To Evaluate Later

- Queue scheduling and QoS papers for weighted round-robin, strict-priority control queues, and credit-based arbitration.
- Accelerator task-graph scheduling papers for dependency tables, graph lowering, and abort propagation.
- Performance modeling papers for abstract-time bottleneck attribution and component budget exploration.

These paper references should be added only when a later PDMA decision needs more than the current specs/code patterns explain.
