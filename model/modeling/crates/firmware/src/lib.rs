//! Device-side management software over injected register access.

use pdma_model_software::ring::SoftRingError;
mod configuration;
mod control;
pub use configuration::Configuration;
pub use control::main;

use hw_model_io::memory::MemoryAccessError;
use hw_model_schema::{RegisterCodecError, SchemaCodecError};
use hw_model_swenv::InterruptRegistrationError;
use pdma_model_contract::ring::RingError;
use std::{error, fmt};

/// Firmware boot or management-service failure.
#[derive(Debug)]
pub enum Error<E, M = MemoryAccessError> {
    /// The platform's Function namespace is empty or cannot be encoded.
    FunctionCount,
    /// DMA recovery is required before this operation can proceed.
    DeviceFailed,
    /// The management access failed.
    Bus(E),
    /// The firmware CPU could not access a control ring entry.
    Memory(M),
    /// A malformed management record was not consumed.
    Codec(SchemaCodecError),
    /// Hardware returned an invalid register encoding.
    RegisterCodec(RegisterCodecError),
    /// The control ring geometry or pointers are invalid.
    Ring(RingError),
    /// A memory-backed software ring operation failed.
    SoftRing(SoftRingError<M, M>),
    /// A shared DMA control operation failed.
    Dma(pdma_model_software::dma::Error<E>),
    /// Firmware could not register its management interrupt handler.
    Registration(InterruptRegistrationError<()>),
    /// Firmware could not allocate its hardware-discovered resource table.
    Allocation(std::collections::TryReserveError),
}

impl<E: fmt::Display, M: fmt::Display> fmt::Display for Error<E, M> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DeviceFailed => formatter.write_str("DMA recovery required"),
            Self::FunctionCount => formatter.write_str("invalid firmware Function count"),
            Self::Bus(error) => write!(formatter, "firmware management access: {error}"),
            Self::Memory(error) => error.fmt(formatter),
            Self::Codec(error) => error.fmt(formatter),
            Self::RegisterCodec(error) => error.fmt(formatter),
            Self::Ring(error) => error.fmt(formatter),
            Self::SoftRing(error) => error.fmt(formatter),
            Self::Dma(error) => error.fmt(formatter),
            Self::Registration(error) => error.fmt(formatter),
            Self::Allocation(error) => error.fmt(formatter),
        }
    }
}

impl<E: error::Error + 'static, M: error::Error + 'static> error::Error for Error<E, M> {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self {
            Self::FunctionCount | Self::DeviceFailed => None,
            Self::Bus(error) => Some(error),
            Self::Memory(error) => Some(error),
            Self::Codec(error) => Some(error),
            Self::RegisterCodec(error) => Some(error),
            Self::Ring(error) => Some(error),
            Self::SoftRing(error) => Some(error),
            Self::Dma(error) => Some(error),
            Self::Registration(error) => Some(error),
            Self::Allocation(error) => Some(error),
        }
    }
}
