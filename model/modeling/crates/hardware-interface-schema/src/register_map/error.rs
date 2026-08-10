use core::fmt;

use crate::register::RegisterValidationError;

/// Failure while validating a register map.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum RegisterMapValidationError<I> {
    /// Two declarations use the same typed or explicit numeric identity.
    DuplicateRegisterId {
        /// Repeated identity.
        id: I,
    },
    /// Two declarations use the same symbolic name.
    DuplicateRegisterName {
        /// Repeated name.
        name: &'static str,
    },
    /// Two expanded registers have the same exact start.
    DuplicateExpandedStart {
        /// Repeated map-relative offset.
        offset: usize,
    },
    /// One register has invalid scalar metadata.
    Register(#[doc = "Underlying register error."] RegisterValidationError),
    /// A fixed or expanded register exceeds the map extent.
    RegisterOutOfBounds {
        /// Register name.
        name: &'static str,
        /// Expanded offset.
        offset: usize,
        /// Byte width.
        width: usize,
        /// Map extent.
        map_size: usize,
    },
    /// A repeated bank has a zero stride.
    BankStrideZero {
        /// Bank name.
        bank_name: &'static str,
    },
    /// A bank stride is not aligned for its widest member.
    BankStrideMisaligned {
        /// Bank name.
        bank_name: &'static str,
        /// Supplied stride.
        stride: usize,
        /// Required alignment.
        required_alignment: usize,
    },
    /// A bank stride cannot contain all member extents.
    BankStrideTooSmall {
        /// Bank name.
        bank_name: &'static str,
        /// Supplied stride.
        stride: usize,
        /// Required extent.
        required_extent: usize,
    },
    /// Expanded bank offset arithmetic overflowed.
    BankExpansionOverflow {
        /// Bank name.
        bank_name: &'static str,
        /// Instance being expanded.
        instance: usize,
        /// Register name.
        register_name: &'static str,
    },
    /// Two expanded register byte ranges intersect.
    ExpandedOverlap {
        /// Earlier register name.
        first_name: &'static str,
        /// Earlier bank instance, if any.
        first_instance: Option<usize>,
        /// Later register name.
        second_name: &'static str,
        /// Later bank instance, if any.
        second_instance: Option<usize>,
    },
}

impl<I: fmt::Debug> fmt::Display for RegisterMapValidationError<I> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid register map: {self:?}")
    }
}

impl<I: fmt::Debug> std::error::Error for RegisterMapValidationError<I> {}
