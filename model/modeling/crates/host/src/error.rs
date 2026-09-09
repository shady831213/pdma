use hardware_interface_runtime::{CpuContextError, MemoryAccessError};
use hardware_interface_schema::SchemaCodecError;
use pdma_model_contract::RingError;
use std::{error::Error as StdError, fmt};

/// Software ring operation failure retaining its boundary cause.
#[derive(Debug)]
pub enum Error<B> {
    /// Register transport rejected an access.
    Bus(B),
    /// Injected memory rejected an access.
    Memory(MemoryAccessError),
    /// A wire record failed its generated codec.
    Codec(SchemaCodecError),
    /// The owning software CPU is not active.
    Cpu(CpuContextError),
    /// A shared ring pointer update is invalid.
    Ring(RingError),
    /// A register address cannot be represented.
    Geometry,
    /// No command slot is currently available.
    Full,
}

impl<B: fmt::Display> fmt::Display for Error<B> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Bus(error) => error.fmt(f),
            Self::Memory(error) => error.fmt(f),
            Self::Codec(error) => error.fmt(f),
            Self::Cpu(error) => error.fmt(f),
            Self::Ring(error) => error.fmt(f),
            Self::Geometry => f.write_str("invalid host register address"),
            Self::Full => f.write_str("command ring is full"),
        }
    }
}

impl<B: StdError + 'static> StdError for Error<B> {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            Self::Bus(error) => Some(error),
            Self::Memory(error) => Some(error),
            Self::Codec(error) => Some(error),
            Self::Cpu(error) => Some(error),
            Self::Ring(error) => Some(error),
            Self::Geometry | Self::Full => None,
        }
    }
}
