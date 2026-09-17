use core::fmt;

use crate::register::{RegisterDefinitionError, RegisterValidationError};

/// Failure while validating the legacy register-map surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum RegisterMapValidationError<I> {
    /// Complete placement envelopes intersect, including reserved holes.
    PlacementEnvelopeOverlap {
        #[doc = "Earlier scalar or bank declaration name."]
        first_name: &'static str,
        #[doc = "Later scalar or bank declaration name."]
        second_name: &'static str,
    },
    /// Two declarations use the same typed or numeric identity.
    DuplicateRegisterId {
        #[doc = "Repeated identity."]
        id: I,
    },
    /// Two declarations use the same symbolic name.
    DuplicateRegisterName {
        #[doc = "Repeated name."]
        name: &'static str,
    },
    /// Two expanded registers have the same exact start.
    DuplicateExpandedStart {
        #[doc = "Repeated offset."]
        offset: usize,
    },
    /// One register has invalid scalar metadata.
    Register(#[doc = "Underlying register error."] RegisterValidationError),
    /// A fixed or expanded register exceeds the map extent.
    RegisterOutOfBounds {
        #[doc = "Register name."]
        name: &'static str,
        #[doc = "Expanded offset."]
        offset: usize,
        #[doc = "Byte width."]
        width: usize,
        #[doc = "Map extent."]
        map_size: usize,
    },
    /// A repeated bank has zero stride.
    BankStrideZero {
        #[doc = "Bank name."]
        bank_name: &'static str,
    },
    /// A bank stride is not aligned for its widest member.
    BankStrideMisaligned {
        #[doc = "Bank name."]
        bank_name: &'static str,
        #[doc = "Supplied stride."]
        stride: usize,
        #[doc = "Required alignment."]
        required_alignment: usize,
    },
    /// A bank stride cannot contain all member extents.
    BankStrideTooSmall {
        #[doc = "Bank name."]
        bank_name: &'static str,
        #[doc = "Supplied stride."]
        stride: usize,
        #[doc = "Required extent."]
        required_extent: usize,
    },
    /// Expanded bank offset arithmetic overflowed.
    BankExpansionOverflow {
        #[doc = "Bank name."]
        bank_name: &'static str,
        #[doc = "Instance index."]
        instance: usize,
        #[doc = "Register name."]
        register_name: &'static str,
    },
    /// Two expanded register byte ranges intersect.
    ExpandedOverlap {
        #[doc = "Earlier register name."]
        first_name: &'static str,
        #[doc = "Earlier bank instance."]
        first_instance: Option<usize>,
        #[doc = "Later register name."]
        second_name: &'static str,
        #[doc = "Later bank instance."]
        second_instance: Option<usize>,
    },
}

impl<I: fmt::Debug> fmt::Display for RegisterMapValidationError<I> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid register map: {self:?}")
    }
}

impl<I: fmt::Debug> std::error::Error for RegisterMapValidationError<I> {}

/// Contextual failure while resolving a named register map.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum RegisterMapError<I = u16, F = u16> {
    /// Complete placement envelopes intersect, including reserved holes.
    PlacementEnvelopeOverlap {
        #[doc = "Root map name."]
        root: &'static str,
        #[doc = "Later scalar or bank declaration path."]
        path: String,
        #[doc = "Later declaration index."]
        declaration_index: usize,
        #[doc = "Earlier scalar or bank declaration path."]
        first_path: String,
    },
    /// Two placement declarations reuse an identity.
    DuplicatePlacementId {
        #[doc = "Root map name."]
        root: &'static str,
        #[doc = "Later placement path."]
        path: String,
        #[doc = "Later declaration index."]
        declaration_index: usize,
        #[doc = "Repeated identity."]
        id: I,
    },
    /// Two placement declarations reuse a name.
    DuplicatePlacementName {
        #[doc = "Root map name."]
        root: &'static str,
        #[doc = "Later placement path."]
        path: String,
        #[doc = "Later declaration index."]
        declaration_index: usize,
        #[doc = "Repeated name."]
        name: &'static str,
    },
    /// Two expanded instances have the same exact start.
    DuplicateExpandedStart {
        #[doc = "Root map name."]
        root: &'static str,
        #[doc = "Later instance path."]
        path: String,
        #[doc = "Later declaration index."]
        declaration_index: usize,
        #[doc = "Repeated offset."]
        offset: usize,
        #[doc = "Earlier instance path."]
        first_path: String,
    },
    /// A referenced definition is invalid.
    Definition(#[doc = "Definition failure."] RegisterDefinitionError<F>),
    /// A fixed or repeated instance exceeds the map extent.
    RegisterOutOfBounds {
        #[doc = "Root map name."]
        root: &'static str,
        #[doc = "Instance path."]
        path: String,
        #[doc = "Placement declaration index."]
        declaration_index: usize,
        #[doc = "Expanded offset."]
        offset: usize,
        #[doc = "Byte width."]
        width: usize,
        #[doc = "Map extent."]
        map_size: usize,
    },
    /// A repeated bank has zero stride.
    BankStrideZero {
        #[doc = "Root map name."]
        root: &'static str,
        #[doc = "Bank path."]
        path: String,
        #[doc = "First member declaration index."]
        declaration_index: usize,
    },
    /// A bank stride is not aligned for its widest member.
    BankStrideMisaligned {
        #[doc = "Root map name."]
        root: &'static str,
        #[doc = "Bank path."]
        path: String,
        #[doc = "First member declaration index."]
        declaration_index: usize,
        #[doc = "Supplied stride."]
        stride: usize,
        #[doc = "Required alignment."]
        required_alignment: usize,
    },
    /// A bank stride cannot contain all member extents.
    BankStrideTooSmall {
        #[doc = "Root map name."]
        root: &'static str,
        #[doc = "Bank path."]
        path: String,
        #[doc = "First member declaration index."]
        declaration_index: usize,
        #[doc = "Supplied stride."]
        stride: usize,
        #[doc = "Required extent."]
        required_extent: usize,
    },
    /// Expanded bank arithmetic overflowed.
    BankExpansionOverflow {
        #[doc = "Root map name."]
        root: &'static str,
        #[doc = "Instance path."]
        path: String,
        #[doc = "Placement declaration index."]
        declaration_index: usize,
        #[doc = "Instance index."]
        instance: usize,
    },
    /// Two expanded byte ranges intersect.
    ExpandedOverlap {
        #[doc = "Root map name."]
        root: &'static str,
        #[doc = "Later instance path."]
        path: String,
        #[doc = "Later declaration index."]
        declaration_index: usize,
        #[doc = "Earlier instance path."]
        first_path: String,
    },
    /// Two resolved instances render the same path.
    ResolvedPathCollision {
        #[doc = "Root map name."]
        root: &'static str,
        #[doc = "Colliding path."]
        path: String,
        #[doc = "Later declaration index."]
        declaration_index: usize,
        #[doc = "Earlier declaration index."]
        first_declaration_index: usize,
    },
}

impl<I: fmt::Debug, F: fmt::Debug> fmt::Display for RegisterMapError<I, F> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid resolved register map: {self:?}")
    }
}

impl<I: fmt::Debug, F: fmt::Debug> std::error::Error for RegisterMapError<I, F> {}
