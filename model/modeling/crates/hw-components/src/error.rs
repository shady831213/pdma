use core::fmt::{self, Display, Formatter};

use hardware_interface_schema::RegisterAccessError;

/// Invalid access-window address geometry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WindowConfigError {
    /// Downstream storage reaches the reserved management addresses.
    ReservedAddressOverlap,
    /// The platform cannot represent the management addresses.
    AddressUnrepresentable,
}

impl Display for WindowConfigError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::ReservedAddressOverlap => "downstream overlaps management addresses",
            Self::AddressUnrepresentable => "management addresses cannot be represented",
        })
    }
}

impl core::error::Error for WindowConfigError {}

/// Rejection of one host transaction through an access window.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AccessError<E> {
    /// Firmware has closed downstream host access.
    Closed,
    /// Host software has no permission for the requested management access.
    Denied,
    /// The complete transaction is outside the mapped downstream range.
    Unmapped,
    /// Generated status-register validation rejected the transaction.
    Register(RegisterAccessError),
    /// The downstream hardware rejected the transaction.
    Downstream(E),
}

impl<E: Display> Display for AccessError<E> {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Closed => formatter.write_str("downstream access is closed"),
            Self::Denied => formatter.write_str("host management access denied"),
            Self::Unmapped => formatter.write_str("transaction outside downstream window"),
            Self::Register(error) => Display::fmt(error, formatter),
            Self::Downstream(error) => Display::fmt(error, formatter),
        }
    }
}

impl<E: core::error::Error + 'static> core::error::Error for AccessError<E> {
    fn source(&self) -> Option<&(dyn core::error::Error + 'static)> {
        match self {
            Self::Register(error) => Some(error),
            Self::Downstream(error) => Some(error),
            Self::Closed | Self::Denied | Self::Unmapped => None,
        }
    }
}
