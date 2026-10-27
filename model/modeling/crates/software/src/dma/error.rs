use crate::ring::SoftRingError;
use hardware_interface_io::memory::MemoryAccessError;
use hardware_interface_schema::{RegisterCodecError, SchemaCodecError};
use pdma_model_contract::dma::ChannelId;
use pdma_model_contract::ring::RingError;
use std::{error::Error as StdError, fmt};

/// Software ring operation failure retaining its boundary cause.
#[derive(Debug)]
pub enum Error<B, M = MemoryAccessError> {
    /// Register transport rejected an access.
    Bus(B),
    /// Injected memory rejected an access.
    Memory(M),
    /// A wire record failed its generated codec.
    Codec(SchemaCodecError),
    /// A register field contains an invalid encoding.
    RegisterCodec(RegisterCodecError),
    /// A shared ring pointer update is invalid.
    Ring(RingError),
    /// The selected channel is outside the hardware-advertised channel count.
    Channel {
        /// Requested channel identifier.
        requested: ChannelId,
        /// Channel count read from hardware.
        available: u32,
    },
    /// A register address cannot be represented.
    Geometry,
    /// No command slot is currently available.
    Full,
    /// An accepted software ring publication must be resumed first.
    RingBusy,
    /// No retained software ring operation is available.
    RingIdle,
    /// The software-owned cursor changed during a retained operation.
    RingCursorChanged,
}

impl<B: fmt::Display, M: fmt::Display> fmt::Display for Error<B, M> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Bus(error) => error.fmt(f),
            Self::Memory(error) => error.fmt(f),
            Self::Codec(error) => error.fmt(f),
            Self::RegisterCodec(error) => error.fmt(f),
            Self::Ring(error) => error.fmt(f),
            Self::Channel {
                requested,
                available,
            } => write!(
                f,
                "channel {} is outside {available} available channels",
                requested.index()
            ),
            Self::Geometry => f.write_str("invalid DMA register address"),
            Self::Full => f.write_str("command ring is full"),
            Self::RingBusy => f.write_str("software ring publication is pending"),
            Self::RingIdle => f.write_str("no retained software ring operation"),
            Self::RingCursorChanged => f.write_str("software ring cursor changed unexpectedly"),
        }
    }
}

impl<B: StdError + 'static, M: StdError + 'static> StdError for Error<B, M> {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            Self::Bus(error) => Some(error),
            Self::Memory(error) => Some(error),
            Self::Codec(error) => Some(error),
            Self::RegisterCodec(error) => Some(error),
            Self::Ring(error) => Some(error),
            Self::Channel { .. }
            | Self::Geometry
            | Self::Full
            | Self::RingBusy
            | Self::RingIdle
            | Self::RingCursorChanged => None,
        }
    }
}

impl<B, M> From<SoftRingError<B, M>> for Error<B, M> {
    fn from(error: SoftRingError<B, M>) -> Self {
        match error {
            SoftRingError::Cursor(error) => Self::Bus(error),
            SoftRingError::Memory(error) => Self::Memory(error),
            SoftRingError::Codec(error) => Self::Codec(error),
            SoftRingError::Ring(error) => Self::Ring(error),
            SoftRingError::Busy => Self::RingBusy,
            SoftRingError::Idle => Self::RingIdle,
            SoftRingError::CursorChanged => Self::RingCursorChanged,
        }
    }
}
