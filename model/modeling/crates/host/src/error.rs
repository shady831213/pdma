use hardware_interface_runtime::{CpuContextError, MemoryAccessError};
use hardware_interface_schema::{RegisterCodecError, SchemaCodecError};
use pdma_model_contract::{ChannelId, ControlResponseStatus, FirmwareBootError, RingError};
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
    /// A register field contains an invalid encoding.
    RegisterCodec(RegisterCodecError),
    /// The owning software CPU is not active.
    Cpu(CpuContextError),
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
    /// Firmware reported an initialization failure through its status register.
    FirmwareFailed(FirmwareBootError),
    /// The Host's deadline expired before firmware reported readiness.
    FirmwareTimeout,
    /// The existing management request must be collected before another submission.
    ManagementBusy,
    /// No management request is available to resume.
    ManagementIdle,
    /// Ring ownership or response correlation was violated.
    ManagementProtocol,
    /// Firmware rejected a completed management operation.
    ManagementStatus(ControlResponseStatus),
}

impl<B: fmt::Display> fmt::Display for Error<B> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Bus(error) => error.fmt(f),
            Self::Memory(error) => error.fmt(f),
            Self::Codec(error) => error.fmt(f),
            Self::RegisterCodec(error) => error.fmt(f),
            Self::Cpu(error) => error.fmt(f),
            Self::Ring(error) => error.fmt(f),
            Self::Channel {
                requested,
                available,
            } => write!(
                f,
                "channel {} is outside {available} available channels",
                requested.index()
            ),
            Self::Geometry => f.write_str("invalid host register address"),
            Self::Full => f.write_str("command ring is full"),
            Self::FirmwareFailed(code) => write!(f, "firmware initialization failed: {code:?}"),
            Self::FirmwareTimeout => f.write_str("timed out waiting for firmware readiness"),
            Self::ManagementBusy => f.write_str("management request is still pending"),
            Self::ManagementIdle => f.write_str("no pending management request"),
            Self::ManagementProtocol => f.write_str("management protocol mismatch"),
            Self::ManagementStatus(status) => write!(f, "management operation: {status:?}"),
        }
    }
}

impl<B: StdError + 'static> StdError for Error<B> {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            Self::Bus(error) => Some(error),
            Self::Memory(error) => Some(error),
            Self::Codec(error) => Some(error),
            Self::RegisterCodec(error) => Some(error),
            Self::Cpu(error) => Some(error),
            Self::Ring(error) => Some(error),
            Self::Channel { .. }
            | Self::Geometry
            | Self::Full
            | Self::FirmwareFailed(_)
            | Self::FirmwareTimeout
            | Self::ManagementBusy
            | Self::ManagementIdle
            | Self::ManagementProtocol
            | Self::ManagementStatus(_) => None,
        }
    }
}
