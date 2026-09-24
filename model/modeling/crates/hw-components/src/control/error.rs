use core::fmt;

/// Rejection of a control-channel register write.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ControlError {
    /// The software domain does not own this register.
    Permission,
    /// Activation prevents changing bootstrap or disabling the control channel.
    ConfigurationLocked,
    /// Enabling requires a defined Default/PASID configuration.
    InvalidAttributes,
}

impl fmt::Display for ControlError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Permission => f.write_str("control register belongs to another domain"),
            Self::ConfigurationLocked => f.write_str("control channel configuration is locked"),
            Self::InvalidAttributes => f.write_str("invalid control context attributes"),
        }
    }
}

impl core::error::Error for ControlError {}
