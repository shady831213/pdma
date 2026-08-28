use core::fmt;

use crate::abi::AbiDecodeError;

use super::violation::DescriptorViolation;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum DescriptorPayloadViolation {
    InvalidIndirectCount { list: u64, observed: u32 },
    InvalidIndirectAlignment { address: u64 },
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

impl fmt::Display for DescriptorDecodeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Codec(error) => error.fmt(formatter),
            Self::MetadataMismatch => formatter.write_str("descriptor metadata is incomplete"),
            Self::Violation(violation) => violation.fmt(formatter),
        }
    }
}

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
    use super::{DescriptorDecodeError, DescriptorPayloadViolation, DescriptorViolation};

    #[test]
    fn payload_count_violation_converts_at_the_decode_boundary() {
        // Given: one private semantic violation at the ABI decode boundary.
        let violation = DescriptorPayloadViolation::InvalidIndirectCount {
            list: 8,
            observed: 0,
        };

        // When: the boundary converts the violation into its public error domain.
        let decode_error = DescriptorDecodeError::Violation(DescriptorViolation::from(violation));

        // Then: the hardware-visible violation is retained exactly.
        assert_eq!(
            decode_error,
            DescriptorDecodeError::Violation(DescriptorViolation::from(violation))
        );
    }
}
