//! PDMA contracts grouped by their owning hardware or software interface.

#[doc = "Host-to-firmware software management messages and memory-resident queues."]
pub mod control;
#[doc = "DMA register, descriptor and outgoing memory-selector contracts."]
pub mod dma;
#[doc = "Peripheral hardware interfaces, independent of the DMA core."]
pub mod hw_components;
#[doc = "Resolved Function and address-space identities carried downstream of hardware lookup."]
pub mod memory;
#[doc = "Register primitives reused by concrete PDMA hardware maps."]
pub mod register;
#[doc = "Shared PDMA ring geometry and cursor protocol."]
pub mod ring;
#[doc = "Host and firmware register-access projections for contract export."]
pub mod view;
