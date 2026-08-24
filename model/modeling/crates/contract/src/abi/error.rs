//! Typed failures at ABI data and control boundaries.

use core::fmt;

mod control;

pub use control::AbiControlError;

/// Failure while constructing a model DMA address capability.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AbiAddressError {
    /// A fixed-width DMA address cannot represent the supplied value.
    UnrepresentableAddress {
        /// Supplied address value.
        value: u128,
    },
    /// The region's exclusive end overflowed the DMA address width.
    RangeOverflow {
        /// Starting DMA address.
        base: u64,
        /// Requested byte length.
        length: u64,
    },
}

/// Failure while encoding host values into fixed-width ABI bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AbiEncodeError {
    /// The destination cannot hold the requested field.
    BufferTooShort {
        /// Minimum destination length.
        required: usize,
        /// Actual destination length.
        actual: usize,
    },
    /// A byte range does not have the required field width.
    InvalidFieldRange {
        /// Supplied start offset.
        start: usize,
        /// Supplied exclusive end.
        end: usize,
        /// Required field width.
        width: usize,
    },
    /// A model identifier does not fit the ABI's `u64` ID width.
    UnrepresentableId {
        /// Supplied identifier value.
        value: u128,
    },
    /// A platform-sized model identifier does not fit the ABI's `u64` ID width.
    UnrepresentableModelId {
        /// Supplied model identifier.
        value: usize,
    },
    /// A semantic-only event has no DMA address fault locator.
    MissingEventDmaAddress,
    /// An event tuple does not follow the ABI-v1 auxiliary-field rules.
    NonCanonicalEventDetails,
    /// Canonical metadata does not describe the required record fields.
    InvalidRecordMetadata,
}

/// Failure while decoding untrusted fixed-width ABI bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AbiDecodeError {
    /// The source does not contain the requested field.
    BufferTooShort {
        /// Minimum source length.
        required: usize,
        /// Actual source length.
        actual: usize,
    },
    /// A byte range does not have the required field width.
    InvalidFieldRange {
        /// Supplied start offset.
        start: usize,
        /// Supplied exclusive end.
        end: usize,
        /// Required field width.
        width: usize,
    },
    /// A discovery feature mask contains bits not assigned by ABI v1.
    UnknownFeatureBits {
        /// Unassigned bits after masking known features.
        bits: u64,
    },
    /// A wire identifier does not fit the model's platform-sized ID vocabulary.
    UnrepresentableId {
        /// Supplied wire identifier.
        value: u64,
    },
    /// A fixed ABI header field has an unsupported value.
    InvalidRecordField {
        /// Canonical field name.
        field: &'static str,
        /// Observed numeric value.
        value: u64,
    },
    /// A completion status is not assigned by ABI v1.
    UnknownCompletionStatus {
        /// Unassigned status value.
        value: u16,
    },
    /// An event kind is not assigned by ABI v1.
    UnknownEventKind {
        /// Unassigned kind value.
        value: u16,
    },
    /// An event stage is not assigned by ABI v1.
    UnknownEventStage {
        /// Unassigned stage value.
        value: u16,
    },
    /// An event access is not assigned by ABI v1.
    UnknownEventAccess {
        /// Unassigned access value.
        value: u16,
    },
    /// An event tuple does not follow the ABI-v1 auxiliary-field rules.
    NonCanonicalEventDetails,
    /// Canonical metadata does not describe the required record fields.
    InvalidRecordMetadata,
}

impl fmt::Display for AbiAddressError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnrepresentableAddress { value } => {
                write!(
                    f,
                    "address {value} is not representable as an ABI DMA address"
                )
            }
            Self::RangeOverflow { base, length } => {
                write!(
                    f,
                    "DMA region {base} + {length} overflows the ABI DMA address width"
                )
            }
        }
    }
}

impl fmt::Display for AbiEncodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BufferTooShort { required, actual } => {
                write!(
                    f,
                    "encoding requires {required} bytes but destination has {actual}"
                )
            }
            Self::InvalidFieldRange { start, end, width } => {
                write!(f, "encoding range {start}..{end} is not {width} bytes wide")
            }
            Self::UnrepresentableId { value } => {
                write!(f, "identifier {value} is not representable as an ABI u64")
            }
            Self::UnrepresentableModelId { value } => {
                write!(
                    f,
                    "model identifier {value} is not representable as an ABI u64"
                )
            }
            Self::MissingEventDmaAddress => write!(f, "event has no ABI DMA address fault locator"),
            Self::NonCanonicalEventDetails => {
                write!(f, "event auxiliary fields are not canonical for ABI v1")
            }
            Self::InvalidRecordMetadata => write!(f, "canonical ABI record metadata is invalid"),
        }
    }
}

impl fmt::Display for AbiDecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BufferTooShort { required, actual } => {
                write!(
                    f,
                    "decoding requires {required} bytes but source has {actual}"
                )
            }
            Self::InvalidFieldRange { start, end, width } => {
                write!(f, "decoding range {start}..{end} is not {width} bytes wide")
            }
            Self::UnknownFeatureBits { bits } => write!(f, "unknown ABI feature bits {bits:#x}"),
            Self::UnrepresentableId { value } => {
                write!(
                    f,
                    "ABI identifier {value} is not representable by the model"
                )
            }
            Self::InvalidRecordField { field, value } => {
                write!(f, "ABI record field {field} has invalid value {value}")
            }
            Self::UnknownCompletionStatus { value } => {
                write!(f, "unknown ABI completion status {value}")
            }
            Self::UnknownEventKind { value } => write!(f, "unknown ABI event kind {value}"),
            Self::UnknownEventStage { value } => write!(f, "unknown ABI event stage {value}"),
            Self::UnknownEventAccess { value } => write!(f, "unknown ABI event access {value}"),
            Self::NonCanonicalEventDetails => {
                write!(f, "event auxiliary fields are not canonical for ABI v1")
            }
            Self::InvalidRecordMetadata => write!(f, "canonical ABI record metadata is invalid"),
        }
    }
}

impl std::error::Error for AbiAddressError {}
impl std::error::Error for AbiEncodeError {}
impl std::error::Error for AbiDecodeError {}
