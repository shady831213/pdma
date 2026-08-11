use core::fmt;

use crate::ScalarKind;

/// Failure while validating a flat fixed-record schema.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum RecordValidationError<I = u16> {
    /// Record alignment is zero.
    AlignmentZero,
    /// Record alignment is not a power of two.
    AlignmentNotPowerOfTwo {
        /// Supplied alignment.
        alignment: usize,
    },
    /// Two fields use the same identifier.
    DuplicateFieldId {
        /// Repeated identifier.
        id: I,
    },
    /// Two fields use the same name.
    DuplicateFieldName {
        /// Repeated name.
        name: &'static str,
    },
    /// A field width differs from its scalar kind.
    ScalarWidthMismatch {
        /// Field identifier.
        field_id: I,
        /// Scalar kind.
        scalar: ScalarKind,
        /// Declared width.
        width: usize,
    },
    /// Two scalar ranges intersect.
    FieldOverlap {
        /// Earlier field identifier.
        first_id: I,
        /// Later field identifier.
        second_id: I,
    },
    /// A scalar extent exceeds the record size or `usize`.
    FieldOutOfBounds {
        /// Field identifier.
        field_id: I,
        /// Local offset.
        offset: usize,
        /// Declared width.
        width: usize,
        /// Containing record size.
        record_size: usize,
    },
}

impl<I: fmt::Debug> fmt::Display for RecordValidationError<I> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid record schema: {self:?}")
    }
}

impl<I: fmt::Debug> std::error::Error for RecordValidationError<I> {}

/// Failure while validating or resolving a nested record graph.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum RecordGraphError<I = u16> {
    /// Combined scalar/nested siblings reuse an identifier.
    DuplicateSiblingId {
        /// Root definition name.
        root: &'static str,
        /// Definition path.
        path: String,
        /// Later sibling declaration index.
        declaration_index: usize,
        /// Repeated identifier.
        id: I,
    },
    /// Combined scalar/nested siblings reuse a name.
    DuplicateSiblingName {
        /// Root definition name.
        root: &'static str,
        /// Definition path.
        path: String,
        /// Later sibling declaration index.
        declaration_index: usize,
        /// Repeated name.
        name: &'static str,
    },
    /// A scalar width differs from its kind.
    ScalarWidthMismatch {
        /// Root definition name.
        root: &'static str,
        /// Field path.
        path: String,
        /// Field declaration index.
        declaration_index: usize,
        /// Field identifier.
        id: I,
        /// Scalar kind.
        scalar: ScalarKind,
        /// Declared width.
        width: usize,
    },
    /// Record alignment is zero.
    AlignmentZero {
        /// Root definition name.
        root: &'static str,
        /// Definition path.
        path: String,
        /// Definition declaration index.
        declaration_index: usize,
    },
    /// Record alignment is not a power of two.
    AlignmentNotPowerOfTwo {
        /// Root definition name.
        root: &'static str,
        /// Definition path.
        path: String,
        /// Definition declaration index.
        declaration_index: usize,
        /// Supplied alignment.
        alignment: usize,
    },
    /// A local scalar extent exceeds its record.
    FieldOutOfBounds {
        /// Root definition name.
        root: &'static str,
        /// Field path.
        path: String,
        /// Field declaration index.
        declaration_index: usize,
        /// Field identifier.
        id: I,
        /// Local offset.
        offset: usize,
        /// Declared width.
        width: usize,
        /// Containing record size.
        record_size: usize,
    },
    /// Two local scalar ranges intersect.
    FieldOverlap {
        /// Root definition name.
        root: &'static str,
        /// Definition path.
        path: String,
        /// Later field declaration index.
        declaration_index: usize,
        /// Earlier field identifier.
        first_id: I,
        /// Later field identifier.
        second_id: I,
    },
    /// A definition is active earlier in the current traversal stack.
    RecursiveRecord {
        /// Root definition name.
        root: &'static str,
        /// Recursive placement path.
        path: String,
        /// Placement declaration index.
        declaration_index: usize,
        /// Re-entered definition name.
        definition: &'static str,
    },
    /// Placement arithmetic overflowed `usize`.
    PlacementOffsetOverflow {
        /// Root definition name.
        root: &'static str,
        /// Placement instance path.
        path: String,
        /// Placement declaration index.
        declaration_index: usize,
        /// Placement base offset.
        offset: usize,
        /// Placement instance.
        instance: usize,
        /// Placement stride.
        stride: usize,
    },
    /// A nested placement exceeds its containing record.
    PlacementOutOfBounds {
        /// Root definition name.
        root: &'static str,
        /// Placement instance path.
        path: String,
        /// Placement declaration index.
        declaration_index: usize,
        /// Computed local offset.
        offset: usize,
        /// Referenced record size.
        size: usize,
        /// Containing record size.
        record_size: usize,
    },
    /// Scalar/nested or nested/nested ranges intersect.
    MemberOverlap {
        /// Root definition name.
        root: &'static str,
        /// Containing definition path.
        path: String,
        /// Later member declaration index.
        declaration_index: usize,
        /// Earlier member path.
        first_path: String,
        /// Later member path.
        second_path: String,
    },
    /// Two distinct resolved members render the same path.
    ResolvedPathCollision {
        /// Root definition name.
        root: &'static str,
        /// Colliding rendered path.
        path: String,
        /// Later member declaration index.
        declaration_index: usize,
        /// Earlier member declaration index.
        first_declaration_index: usize,
    },
}

impl<I: fmt::Debug> fmt::Display for RecordGraphError<I> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid resolved record graph: {self:?}")
    }
}

impl<I: fmt::Debug> std::error::Error for RecordGraphError<I> {}
