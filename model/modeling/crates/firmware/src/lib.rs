//! Device-side management software over injected register access.

use pdma_model_software::ring::SoftRingError;
mod control;
pub use control::main;

use hardware_interface_runtime::{InterruptRegistrationError, MemoryAccessError};
use hardware_interface_schema::SchemaCodecError;
use pdma_model_contract::RingError;
use std::{error, fmt};

/// Firmware boot or management-service failure.
#[derive(Debug)]
pub enum Error<E> {
    /// The management access failed.
    Bus(E),
    /// The firmware CPU could not access a control ring entry.
    Memory(MemoryAccessError),
    /// A malformed management record was not consumed.
    Codec(SchemaCodecError),
    /// The control ring geometry or pointers are invalid.
    Ring(RingError),
    /// A memory-backed software ring operation failed.
    SoftRing(SoftRingError<MemoryAccessError>),
    /// A shared DMA control operation failed.
    Dma(pdma_model_software::dma::Error<E>),
    /// Firmware could not register its management interrupt handler.
    Registration(InterruptRegistrationError<()>),
    /// Firmware could not allocate its hardware-discovered resource table.
    Allocation(std::collections::TryReserveError),
}

impl<E: fmt::Display> fmt::Display for Error<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Bus(error) => write!(formatter, "firmware management access: {error}"),
            Self::Memory(error) => error.fmt(formatter),
            Self::Codec(error) => error.fmt(formatter),
            Self::Ring(error) => error.fmt(formatter),
            Self::SoftRing(error) => error.fmt(formatter),
            Self::Dma(error) => error.fmt(formatter),
            Self::Registration(error) => error.fmt(formatter),
            Self::Allocation(error) => error.fmt(formatter),
        }
    }
}

impl<E: error::Error + 'static> error::Error for Error<E> {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self {
            Self::Bus(error) => Some(error),
            Self::Memory(error) => Some(error),
            Self::Codec(error) => Some(error),
            Self::Ring(error) => Some(error),
            Self::SoftRing(error) => Some(error),
            Self::Dma(error) => Some(error),
            Self::Registration(error) => Some(error),
            Self::Allocation(error) => Some(error),
        }
    }
}
