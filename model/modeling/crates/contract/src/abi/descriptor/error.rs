use core::fmt;

use crate::abi::{AbiDecodeError, INDIRECT_ENTRY_ALIGNMENT, MAX_INDIRECT_DESTINATIONS};

use super::violation::DescriptorViolation;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum DescriptorPayloadViolation {
    InvalidIndirectCount { list: u64, observed: u32 },
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
    /// The descriptor is structurally decodable but violates the hardware ABI.
    Violation(DescriptorViolation),
}

impl From<AbiDecodeError> for DescriptorDecodeError {
    fn from(error: AbiDecodeError) -> Self {
        Self::Codec(error)
    }
}

impl From<DescriptorPayloadViolation> for DescriptorValueError {
    fn from(violation: DescriptorPayloadViolation) -> Self {
        match violation {
            DescriptorPayloadViolation::InvalidIndirectCount { observed, .. } => {
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
            Self::Violation(violation) => violation.fmt(formatter),
        }
    }
}

impl std::error::Error for DescriptorValueError {}
impl std::error::Error for DescriptorDecodeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Codec(error) => Some(error),
            Self::Violation(violation) => Some(violation),
            Self::MetadataMismatch => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::abi::MAX_INDIRECT_DESTINATIONS;

    use super::{
        DescriptorDecodeError, DescriptorPayloadViolation, DescriptorValueError,
        DescriptorViolation,
    };

    #[test]
    fn payload_count_violation_converts_at_each_boundary() {
        // Given: one private semantic violation shared across two ABI boundaries.
        let violation = DescriptorPayloadViolation::InvalidIndirectCount {
            list: 8,
            observed: 0,
        };

        // When: each boundary converts the same violation into its public error domain.
        let value_error = DescriptorValueError::from(violation);
        let decode_error = DescriptorDecodeError::Violation(DescriptorViolation::from(violation));

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
            DescriptorDecodeError::Violation(DescriptorViolation::from(violation))
        );
    }
}
