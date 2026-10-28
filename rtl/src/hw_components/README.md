# Managed peripheral hardware

Own Function access windows, context binding/resolution and the hardware-facing
management, boot-status and doorbell registers. Compose these around DMA without
making the direct DMA configuration depend on firmware management.

Resource allocation and lifecycle policy remain firmware responsibilities.
PCIe and MMU implementations belong to the integration platform. This directory
currently contains no implemented RTL.
