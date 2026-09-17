use core::fmt;

/// Rejection of a control-channel register write.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ControlError {
    /// The software domain does not own this register.
    Permission,
    /// Activation prevents changing bootstrap or disabling the control channel.
    ConfigurationLocked,
}

impl fmt::Display for ControlError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Permission => f.write_str("control register belongs to another domain"),
            Self::ConfigurationLocked => f.write_str("control channel configuration is locked"),
        }
    }
}

impl core::error::Error for ControlError {}
