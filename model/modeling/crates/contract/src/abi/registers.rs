//! Direct PDMA hardware register layouts and dynamic queue addressing.

mod access;
mod address;
mod error;
/// Device-wide discovery and reset registers.
pub mod global;
/// Per-source interrupt registers.
pub mod irq;
/// Per-queue configuration and cursor registers.
pub mod queue;
/// Bit assignments for the direct IRQ source status register.
pub mod status;

use hardware_interface_schema::register_map;

pub use access::{decode_read, decode_write};
pub use address::{RegisterAddressError, RegisterLocation, irq_offset, locate, queue_offset};
pub use error::{MmioAccessError, MmioByteCount};
pub use global::{GlobalRegisters, GlobalRegistersRegisterId, GlobalRegistersValues};
pub use irq::{IrqSourceRegisters, IrqSourceRegistersRegisterId, IrqSourceRegistersValues};
pub use queue::{QueueRegisters, QueueRegistersRegisterId, QueueRegistersValues};
pub use status::{IrqSourceStatus, IrqSourceStatusError, IrqStateBit};

register_map! { pub map QueueInstanceRegisters {
    alignment: 8,
    placements: [
        pub queue: QueueRegisters,
        pub sources: IrqSourceRegisters { count: 2 },
    ],
} }

register_map! { pub map PdmaRegisterMap {
    alignment: 8,
    placements: [
        pub global: GlobalRegisters,
        pub queues: QueueInstanceRegisters,
    ],
} }
