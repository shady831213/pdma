//! Concrete PDMA peripheral hardware objects.

/// Firmware-owned context bindings and maintenance completion.
pub mod context;
/// Control-channel bootstrap, permissions, and notifications.
pub mod control;
/// Firmware boot status visible to Host software.
pub mod firmware;
/// Firmware-controlled DMA channel access windows.
pub mod windows;
