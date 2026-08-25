use core::fmt;

use crate::abi::{AbiDecodeError, INDIRECT_ENTRY_ALIGNMENT, MAX_INDIRECT_DESTINATIONS};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum DescriptorPayloadViolation {
    InvalidIndirectCount { observed: u32 },
    InvalidIndirectAlignment { address: u64 },
}

/// Invalid host-side descriptor construction values.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DescriptorValueError {
    /// The indirect destination count is zero or above the advertised maximum.
    InvalidIndirectCount {
        /// Supplied count.
        observed: u32,
        /// Advertised maximum.
        maximum: u32,
    },
    /// The indirect-list DMA address does not meet the entry alignment.
    InvalidIndirectAlignment {
        /// Supplied list DMA address.
        address: u64,
        /// Required alignment.
        alignment: u64,
    },
}

/// Failure while decoding an untrusted command descriptor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DescriptorDecodeError {
    /// A checked scalar codec rejected the source.
    Codec(AbiDecodeError),
    /// Canonical descriptor metadata does not contain the required fields.
    MetadataMismatch,
    /// Descriptor version differs from ABI v1.
    InvalidVersion {
        /// Supplied version.
        observed: u16,
        /// Required version.
        expected: u16,
    },
    /// Descriptor record size differs from 64 bytes.
    InvalidSize {
        /// Supplied size.
        observed: u16,
        /// Required size.
        expected: u16,
    },
    /// Descriptor opcode is not assigned by ABI v1.
    InvalidOpcode {
        /// Supplied opcode.
        observed: u16,
        /// Highest contiguous ABI-v1 opcode.
        maximum: u16,
    },
    /// Descriptor flags contain bits not accepted by ABI v1.
    InvalidFlags {
        /// Supplied flags.
        observed: u16,
        /// Accepted flag mask.
        allowed: u16,
    },
    /// An operation-reserved payload word is nonzero.
    InvalidReserved {
        /// Reserved word byte offset.
        offset: u16,
        /// Supplied word.
        observed: u64,
    },
    /// A fixed-width scalar exceeds its advertised ABI-v1 limit.
    ValueOutOfRange {
        /// Scalar byte offset.
        offset: u16,
        /// Supplied value.
        observed: u64,
        /// Advertised maximum.
        maximum: u32,
    },
    /// The decoded indirect destination count is invalid.
    InvalidIndirectCount {
        /// Supplied count.
        observed: u64,
        /// Advertised maximum.
        maximum: u32,
    },
    /// The decoded indirect-list DMA address is unaligned.
    InvalidIndirectAlignment {
        /// Supplied list DMA address.
        address: u64,
        /// Required alignment.
        alignment: u64,
    },
}

impl From<AbiDecodeError> for DescriptorDecodeError {
    fn from(error: AbiDecodeError) -> Self {
        Self::Codec(error)
    }
}

impl From<DescriptorPayloadViolation> for DescriptorValueError {
    fn from(violation: DescriptorPayloadViolation) -> Self {
        match violation {
            DescriptorPayloadViolation::InvalidIndirectCount { observed } => {
                Self::InvalidIndirectCount {
                    observed,
                    maximum: MAX_INDIRECT_DESTINATIONS,
                }
            }
            DescriptorPayloadViolation::InvalidIndirectAlignment { address } => {
                Self::InvalidIndirectAlignment {
                    address,
                    alignment: INDIRECT_ENTRY_ALIGNMENT,
                }
            }
        }
    }
}

impl From<DescriptorPayloadViolation> for DescriptorDecodeError {
    fn from(violation: DescriptorPayloadViolation) -> Self {
        match violation {
            DescriptorPayloadViolation::InvalidIndirectCount { observed } => {
                Self::InvalidIndirectCount {
                    observed: u64::from(observed),
                    maximum: MAX_INDIRECT_DESTINATIONS,
                }
            }
            DescriptorPayloadViolation::InvalidIndirectAlignment { address } => {
                Self::InvalidIndirectAlignment {
                    address,
                    alignment: INDIRECT_ENTRY_ALIGNMENT,
                }
            }
        }
    }
}

impl fmt::Display for DescriptorValueError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidIndirectCount { observed, maximum } => {
                write!(
                    formatter,
                    "indirect destination count {observed} is outside 1..={maximum}"
                )
            }
            Self::InvalidIndirectAlignment { address, alignment } => {
                write!(
                    formatter,
                    "indirect list address {address:#x} is not aligned to {alignment}"
                )
            }
        }
    }
}

impl fmt::Display for DescriptorDecodeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Codec(error) => error.fmt(formatter),
            Self::MetadataMismatch => formatter.write_str("descriptor metadata is incomplete"),
            Self::InvalidVersion { observed, expected } => {
                write!(formatter, "descriptor version {observed} is not {expected}")
            }
            Self::InvalidSize { observed, expected } => {
                write!(formatter, "descriptor size {observed} is not {expected}")
            }
            Self::InvalidOpcode { observed, maximum } => {
                write!(
                    formatter,
                    "descriptor opcode {observed} is outside 1..={maximum}"
                )
            }
            Self::InvalidFlags { observed, allowed } => {
                write!(
                    formatter,
                    "descriptor flags {observed:#x} exceed mask {allowed:#x}"
                )
            }
            Self::InvalidReserved { offset, observed } => {
                write!(
                    formatter,
                    "descriptor reserved word at {offset} is {observed:#x}"
                )
            }
            Self::ValueOutOfRange {
                offset,
                observed,
                maximum,
            } => write!(
                formatter,
                "descriptor value {observed} at {offset} exceeds {maximum}"
            ),
            Self::InvalidIndirectCount { observed, maximum } => write!(
                formatter,
                "indirect destination count {observed} is outside 1..={maximum}"
            ),
            Self::InvalidIndirectAlignment { address, alignment } => write!(
                formatter,
                "indirect list address {address:#x} is not aligned to {alignment}"
            ),
        }
    }
}

impl std::error::Error for DescriptorValueError {}
impl std::error::Error for DescriptorDecodeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Codec(error) => Some(error),
            Self::MetadataMismatch
            | Self::InvalidVersion { .. }
            | Self::InvalidSize { .. }
            | Self::InvalidOpcode { .. }
            | Self::InvalidFlags { .. }
            | Self::InvalidReserved { .. }
            | Self::ValueOutOfRange { .. }
            | Self::InvalidIndirectCount { .. }
            | Self::InvalidIndirectAlignment { .. } => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::abi::MAX_INDIRECT_DESTINATIONS;

    use super::{DescriptorDecodeError, DescriptorPayloadViolation, DescriptorValueError};

    #[test]
    fn payload_count_violation_converts_at_each_boundary() {
        // Given: one private semantic violation shared across two ABI boundaries.
        let violation = DescriptorPayloadViolation::InvalidIndirectCount { observed: 0 };

        // When: each boundary converts the same violation into its public error domain.
        let value_error = DescriptorValueError::from(violation);
        let decode_error = DescriptorDecodeError::from(violation);

        // Then: each boundary retains its established count representation.
        assert_eq!(
            value_error,
            DescriptorValueError::InvalidIndirectCount {
                observed: 0,
                maximum: MAX_INDIRECT_DESTINATIONS,
            }
        );
        assert_eq!(
            decode_error,
            DescriptorDecodeError::InvalidIndirectCount {
                observed: 0,
                maximum: MAX_INDIRECT_DESTINATIONS,
            }
        );
    }
}
