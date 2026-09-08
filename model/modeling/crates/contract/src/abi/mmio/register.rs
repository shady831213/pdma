/// Global-register definitions and composable block.
pub mod global;
/// IRQ-register definitions and composable block.
pub mod irq;
/// Queue-register definitions and composable block.
pub mod queue;

use hardware_interface_schema::register_map;

pub use hardware_interface_schema::register::{Access as MmioAccess, ResetKind as MmioReset};

register_map! {
    pub map PdmaMmioMap {
        alignment: 8,
        extent: 0x1000,
        placements: [
            pub global: global::GlobalRegisters { offset: 0x000 },
            pub queue: queue::QueueRegisters { offset: 0x100 },
            pub irq: irq::IrqRegisters { offset: 0x200 },
        ],
    }
}

/// Lossless domain for raw MMIO transaction byte counts.
pub type MmioByteCount = u128;

/// Exact byte size derived from the generated MMIO map.
pub const MMIO_REGION_SIZE: MmioByteCount = PdmaMmioMap::EXTENT as MmioByteCount;

/// Canonical generated placement metadata for one PDMA register.
pub type RegisterSpec = hardware_interface_schema::register::RegisterSpec<PdmaMmioMapRegisterId>;
