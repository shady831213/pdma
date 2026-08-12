use crate::abi::{AbiAddressError, AbiDecodeError, AbiEncodeError, DescriptorDecodeError};
use core::fmt;

/// Failure while purely converting semantic, runtime, and exact ABI values.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BridgeError {
    /// ABI encoding rejected a converted semantic value.
    AbiEncode(AbiEncodeError),
    /// ABI decoding rejected an exact value.
    AbiDecode(AbiDecodeError),
    /// ABI address construction rejected a runtime address.
    AbiAddress(AbiAddressError),
    /// Exact descriptor validation rejected input bytes.
    Descriptor(DescriptorDecodeError),
    /// An event conversion omitted its required exact fault locator.
    MissingEventLocator,
    /// The exact descriptor opcode does not match the requested semantic conversion.
    UnexpectedOpcode,
    /// A platform-sized semantic scalar cannot represent the exact ABI value.
    SemanticRange {
        /// Exact value rejected by the semantic representation.
        value: u64,
    },
}

impl From<AbiEncodeError> for BridgeError {
    fn from(error: AbiEncodeError) -> Self {
        Self::AbiEncode(error)
    }
}

impl From<AbiDecodeError> for BridgeError {
    fn from(error: AbiDecodeError) -> Self {
        Self::AbiDecode(error)
    }
}

impl From<AbiAddressError> for BridgeError {
    fn from(error: AbiAddressError) -> Self {
        Self::AbiAddress(error)
    }
}

impl From<DescriptorDecodeError> for BridgeError {
    fn from(error: DescriptorDecodeError) -> Self {
        Self::Descriptor(error)
    }
}

impl fmt::Display for BridgeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AbiEncode(error) => error.fmt(formatter),
            Self::AbiDecode(error) => error.fmt(formatter),
            Self::AbiAddress(error) => error.fmt(formatter),
            Self::Descriptor(error) => error.fmt(formatter),
            Self::MissingEventLocator => formatter.write_str("event has no exact fault locator"),
            Self::UnexpectedOpcode => formatter.write_str("exact descriptor opcode is unexpected"),
            Self::SemanticRange { value } => {
                write!(
                    formatter,
                    "ABI value {value} exceeds the semantic scalar range"
                )
            }
        }
    }
}

impl std::error::Error for BridgeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::AbiEncode(error) => Some(error),
            Self::AbiDecode(error) => Some(error),
            Self::AbiAddress(error) => Some(error),
            Self::Descriptor(error) => Some(error),
            Self::MissingEventLocator | Self::UnexpectedOpcode | Self::SemanticRange { .. } => None,
        }
    }
}
