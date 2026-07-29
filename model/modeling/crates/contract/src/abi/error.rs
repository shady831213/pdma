//! Typed failures at ABI data and control boundaries.

use core::fmt;

/// Failure while constructing a model DMA address capability.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AbiAddressError {
    /// A fixed-width IOVA cannot represent the supplied value.
    UnrepresentableAddress {
        /// Supplied address value.
        value: u128,
    },
    /// The region's exclusive end overflowed the IOVA width.
    RangeOverflow {
        /// Starting IOVA.
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
}

/// Failure while validating a typed ABI control request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AbiControlError {
    /// The requested ABI version is not supported.
    UnsupportedVersion {
        /// Requested ABI version.
        requested: u16,
    },
    /// A control request contains feature bits not assigned by ABI v1.
    UnknownFeatureBits {
        /// Unassigned bits after masking known features.
        bits: u64,
    },
}

impl fmt::Display for AbiAddressError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnrepresentableAddress { value } => {
                write!(f, "address {value} is not representable as an ABI IOVA")
            }
            Self::RangeOverflow { base, length } => {
                write!(
                    f,
                    "DMA region {base} + {length} overflows the ABI IOVA width"
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
        }
    }
}

impl fmt::Display for AbiControlError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedVersion { requested } => {
                write!(f, "ABI version {requested} is not supported")
            }
            Self::UnknownFeatureBits { bits } => {
                write!(
                    f,
                    "control request contains unknown ABI feature bits {bits:#x}"
                )
            }
        }
    }
}

impl std::error::Error for AbiAddressError {}
impl std::error::Error for AbiEncodeError {}
impl std::error::Error for AbiDecodeError {}
impl std::error::Error for AbiControlError {}
