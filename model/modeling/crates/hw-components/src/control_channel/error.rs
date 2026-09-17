use core::fmt;
use pdma_model_contract::RingError;

/// Rejection of a control-channel register write.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ControlError {
    /// The software domain does not own this register.
    Permission,
    /// Activation prevents changing geometry or disabling the control channel.
    ConfigurationLocked,
    /// Pointer writes require an active channel.
    Disabled,
    /// Shared ring geometry or pointer validation failed.
    Ring(RingError),
}

impl fmt::Display for ControlError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Permission => f.write_str("control register belongs to another domain"),
            Self::ConfigurationLocked => f.write_str("control channel configuration is locked"),
            Self::Disabled => f.write_str("control channel is disabled"),
            Self::Ring(error) => error.fmt(f),
        }
    }
}

impl core::error::Error for ControlError {
    fn source(&self) -> Option<&(dyn core::error::Error + 'static)> {
        match self {
            Self::Ring(error) => Some(error),
            Self::Permission | Self::ConfigurationLocked | Self::Disabled => None,
        }
    }
}
