//! Device-side management software over injected register access.

mod control;
pub use control::main;

use hardware_interface_runtime::{CpuContextError, MemoryAccessError};
use hardware_interface_schema::SchemaCodecError;
use pdma_model_contract::RingError;
use std::{error, fmt};

/// Firmware boot or management-service failure.
#[derive(Debug)]
pub enum Error<E> {
    /// Firmware was polled outside its owning software CPU.
    Cpu(CpuContextError),
    /// The management access failed.
    Bus(E),
    /// The firmware CPU could not access a control ring entry.
    Memory(MemoryAccessError),
    /// A malformed management record was not consumed.
    Codec(SchemaCodecError),
    /// The control ring geometry or pointers are invalid.
    Ring(RingError),
}

impl<E: fmt::Display> fmt::Display for Error<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Cpu(error) => write!(formatter, "firmware CPU: {error}"),
            Self::Bus(error) => write!(formatter, "firmware management access: {error}"),
            Self::Memory(error) => error.fmt(formatter),
            Self::Codec(error) => error.fmt(formatter),
            Self::Ring(error) => error.fmt(formatter),
        }
    }
}

impl<E: error::Error + 'static> error::Error for Error<E> {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self {
            Self::Cpu(error) => Some(error),
            Self::Bus(error) => Some(error),
            Self::Memory(error) => Some(error),
            Self::Codec(error) => Some(error),
            Self::Ring(error) => Some(error),
        }
    }
}
