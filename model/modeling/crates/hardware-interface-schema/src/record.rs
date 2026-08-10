use core::{fmt, ops::Range};

use crate::{Endian, ScalarKind};

/// One scalar field in a fixed-width record.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FieldSpec {
    id: u16,
    name: &'static str,
    offset: usize,
    width: usize,
    scalar: ScalarKind,
    endian: Endian,
}

impl FieldSpec {
    /// Creates a little-endian field whose width matches `scalar`.
    pub const fn new(id: u16, name: &'static str, offset: usize, scalar: ScalarKind) -> Self {
        Self {
            id,
            name,
            offset,
            width: scalar.width(),
            scalar,
            endian: Endian::Little,
        }
    }

    /// Selects the field's byte order.
    #[must_use]
    pub const fn with_endian(mut self, endian: Endian) -> Self {
        self.endian = endian;
        self
    }

    /// Overrides the declared width for schema construction and validation.
    #[must_use]
    pub const fn with_width(mut self, width: usize) -> Self {
        self.width = width;
        self
    }

    /// Returns the stable field identifier.
    pub const fn id(self) -> u16 {
        self.id
    }

    /// Returns the symbolic field name.
    pub const fn name(self) -> &'static str {
        self.name
    }

    /// Returns the starting byte offset.
    pub const fn offset(self) -> usize {
        self.offset
    }

    /// Returns the declared field width in bytes.
    pub const fn width(self) -> usize {
        self.width
    }

    /// Returns the scalar kind.
    pub const fn scalar(self) -> ScalarKind {
        self.scalar
    }

    /// Returns the field's byte order.
    pub const fn endian(self) -> Endian {
        self.endian
    }

    /// Returns the field's byte range when its extent is representable.
    pub fn range(self) -> Option<Range<usize>> {
        self.offset
            .checked_add(self.width)
            .map(|end| self.offset..end)
    }
}

/// A fixed record size, alignment, and ordered field table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RecordSpec {
    size: usize,
    alignment: usize,
    fields: &'static [FieldSpec],
}

impl RecordSpec {
    /// Creates an unvalidated const-friendly record description.
    pub const fn new(size: usize, alignment: usize, fields: &'static [FieldSpec]) -> Self {
        Self {
            size,
            alignment,
            fields,
        }
    }

    /// Returns the record width in bytes.
    pub const fn size(self) -> usize {
        self.size
    }

    /// Returns the required alignment in bytes.
    pub const fn alignment(self) -> usize {
        self.alignment
    }

    /// Returns the fields in declaration order.
    pub const fn fields(self) -> &'static [FieldSpec] {
        self.fields
    }

    /// Validates the record with global passes in documented deterministic error order.
    ///
    /// Duplicate IDs precede duplicate names, then scalar widths, record alignment,
    /// field extents, and overlaps. Each pass checks fields in declaration order.
    pub fn validate(self) -> Result<(), RecordValidationError> {
        self.validate_duplicate_ids()?;
        self.validate_duplicate_names()?;
        self.validate_scalar_widths()?;
        self.validate_alignment()?;
        self.validate_field_extents()?;
        self.validate_overlaps()
    }

    fn validate_duplicate_ids(self) -> Result<(), RecordValidationError> {
        for (index, field) in self.fields.iter().copied().enumerate() {
            for previous in self.fields.iter().copied().take(index) {
                if previous.id == field.id {
                    return Err(RecordValidationError::DuplicateFieldId { id: field.id });
                }
            }
        }
        Ok(())
    }

    fn validate_duplicate_names(self) -> Result<(), RecordValidationError> {
        for (index, field) in self.fields.iter().copied().enumerate() {
            for previous in self.fields.iter().copied().take(index) {
                if previous.name == field.name {
                    return Err(RecordValidationError::DuplicateFieldName { name: field.name });
                }
            }
        }
        Ok(())
    }

    fn validate_scalar_widths(self) -> Result<(), RecordValidationError> {
        for field in self.fields.iter().copied() {
            if field.width != field.scalar.width() {
                return Err(RecordValidationError::ScalarWidthMismatch {
                    field_id: field.id,
                    scalar: field.scalar,
                    width: field.width,
                });
            }
        }
        Ok(())
    }

    const fn validate_alignment(self) -> Result<(), RecordValidationError> {
        if self.alignment == 0 {
            return Err(RecordValidationError::AlignmentZero);
        }
        if !self.alignment.is_power_of_two() {
            return Err(RecordValidationError::AlignmentNotPowerOfTwo {
                alignment: self.alignment,
            });
        }
        Ok(())
    }

    fn validate_field_extents(self) -> Result<(), RecordValidationError> {
        for field in self.fields.iter().copied() {
            let Some(end) = field.offset.checked_add(field.width) else {
                return Err(out_of_bounds(field, self.size));
            };
            if end > self.size {
                return Err(out_of_bounds(field, self.size));
            }
        }
        Ok(())
    }

    fn validate_overlaps(self) -> Result<(), RecordValidationError> {
        for (index, field) in self.fields.iter().copied().enumerate() {
            for previous in self.fields.iter().copied().take(index) {
                if ranges_overlap(previous, field) {
                    return Err(RecordValidationError::FieldOverlap {
                        first_id: previous.id,
                        second_id: field.id,
                    });
                }
            }
        }
        Ok(())
    }
}

const fn ranges_overlap(first: FieldSpec, second: FieldSpec) -> bool {
    let first_end = first.offset.saturating_add(first.width);
    let second_end = second.offset.saturating_add(second.width);
    first.offset < second_end && second.offset < first_end
}

const fn out_of_bounds(field: FieldSpec, record_size: usize) -> RecordValidationError {
    RecordValidationError::FieldOutOfBounds {
        field_id: field.id,
        offset: field.offset,
        width: field.width,
        record_size,
    }
}

/// Failure while validating a fixed-record schema.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum RecordValidationError {
    /// Record alignment is zero.
    AlignmentZero,
    /// Record alignment is not a power of two.
    AlignmentNotPowerOfTwo {
        /// Supplied alignment.
        alignment: usize,
    },
    /// Two fields use the same numeric identifier.
    DuplicateFieldId {
        /// Repeated identifier.
        id: u16,
    },
    /// Two fields use the same symbolic name.
    DuplicateFieldName {
        /// Repeated name.
        name: &'static str,
    },
    /// A field's declared width differs from its scalar kind.
    ScalarWidthMismatch {
        /// Field identifier.
        field_id: u16,
        /// Declared scalar kind.
        scalar: ScalarKind,
        /// Declared width.
        width: usize,
    },
    /// Two field byte ranges intersect.
    FieldOverlap {
        /// Earlier field identifier.
        first_id: u16,
        /// Later field identifier.
        second_id: u16,
    },
    /// A field's exclusive extent exceeds the record size or `usize`.
    FieldOutOfBounds {
        /// Field identifier.
        field_id: u16,
        /// Starting byte offset.
        offset: usize,
        /// Declared field width.
        width: usize,
        /// Record width in bytes.
        record_size: usize,
    },
}

impl fmt::Display for RecordValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid record schema: {self:?}")
    }
}

impl std::error::Error for RecordValidationError {}
