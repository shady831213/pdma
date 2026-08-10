use crate::{Endian, FieldSpec, RecordSpec, ScalarKind};

use super::{FragmentModelError, identity::validate_rows};

/// One keyed fixed-record row consumed by C-header renderers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RecordCHeaderField {
    id: u16,
    name: &'static str,
    offset: usize,
    width: usize,
    scalar: ScalarKind,
    endian: Endian,
}

impl RecordCHeaderField {
    /// Projects one schema field into typed C-header metadata.
    #[must_use]
    pub const fn from_spec(field: FieldSpec) -> Self {
        Self {
            id: field.id(),
            name: field.name(),
            offset: field.offset(),
            width: field.width(),
            scalar: field.scalar(),
            endian: field.endian(),
        }
    }

    /// Returns the stable field identity.
    pub const fn id(self) -> u16 {
        self.id
    }

    /// Returns the schema field name.
    pub const fn name(self) -> &'static str {
        self.name
    }

    /// Returns the starting byte offset.
    pub const fn offset(self) -> usize {
        self.offset
    }

    /// Returns the scalar byte width.
    pub const fn width(self) -> usize {
        self.width
    }

    /// Returns the scalar kind.
    pub const fn scalar(self) -> ScalarKind {
        self.scalar
    }

    /// Returns the field byte order.
    pub const fn endian(self) -> Endian {
        self.endian
    }
}

/// Complete ordered typed model for one fixed-record C fragment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordCHeaderModel {
    size: usize,
    alignment: usize,
    fields: Vec<RecordCHeaderField>,
}

impl RecordCHeaderModel {
    /// Projects a schema into a complete typed C fragment model.
    #[must_use]
    pub fn from_spec(schema: RecordSpec) -> Self {
        Self {
            size: schema.size(),
            alignment: schema.alignment(),
            fields: schema
                .fields()
                .iter()
                .copied()
                .map(RecordCHeaderField::from_spec)
                .collect(),
        }
    }

    /// Validates caller-supplied rows against their source schema.
    ///
    /// # Errors
    /// Returns the first duplicate, unknown, missing, reordered, or mismatched row.
    pub fn try_from_fields(
        schema: RecordSpec,
        fields: Vec<RecordCHeaderField>,
    ) -> Result<Self, FragmentModelError> {
        let expected = Self::from_spec(schema);
        validate_rows(&expected.fields, &fields, |field| field.id())?;
        Ok(Self {
            size: expected.size,
            alignment: expected.alignment,
            fields,
        })
    }

    /// Returns the record byte size.
    pub const fn size(&self) -> usize {
        self.size
    }

    /// Returns the record byte alignment.
    pub const fn alignment(&self) -> usize {
        self.alignment
    }

    /// Returns every field in canonical declaration order.
    pub fn fields(&self) -> &[RecordCHeaderField] {
        &self.fields
    }
}

#[cfg(test)]
mod tests {
    use crate::{FieldSpec, RecordSpec, ScalarKind};

    use super::{FragmentModelError, RecordCHeaderField, RecordCHeaderModel};

    const FIELDS: [FieldSpec; 2] = [
        FieldSpec::new(1, "first", 0, ScalarKind::U16),
        FieldSpec::new(2, "second", 2, ScalarKind::U32),
    ];
    const SCHEMA: RecordSpec = RecordSpec::new(6, 2, &FIELDS);

    #[test]
    fn missing_typed_row_is_rejected() {
        let mut rows = RecordCHeaderModel::from_spec(SCHEMA).fields;
        rows.pop();
        assert_eq!(
            RecordCHeaderModel::try_from_fields(SCHEMA, rows),
            Err(FragmentModelError::MissingIdentity { expected_index: 1 })
        );
    }

    #[test]
    fn duplicate_typed_identity_is_rejected() {
        let mut rows = RecordCHeaderModel::from_spec(SCHEMA).fields;
        let first = rows.remove(0);
        rows = vec![first, first];
        assert_eq!(
            RecordCHeaderModel::try_from_fields(SCHEMA, rows),
            Err(FragmentModelError::DuplicateIdentity {
                first_index: 0,
                duplicate_index: 1
            })
        );
    }

    #[test]
    fn unknown_typed_identity_is_rejected() {
        let mut rows = RecordCHeaderModel::from_spec(SCHEMA).fields;
        rows.pop();
        rows.push(RecordCHeaderField::from_spec(FieldSpec::new(
            99,
            "unknown",
            2,
            ScalarKind::U32,
        )));
        assert_eq!(
            RecordCHeaderModel::try_from_fields(SCHEMA, rows),
            Err(FragmentModelError::UnknownIdentity { actual_index: 1 })
        );
    }

    #[test]
    fn reordered_typed_rows_are_rejected() {
        let mut rows = RecordCHeaderModel::from_spec(SCHEMA).fields;
        rows.swap(0, 1);
        assert_eq!(
            RecordCHeaderModel::try_from_fields(SCHEMA, rows),
            Err(FragmentModelError::ReorderedIdentity {
                actual_index: 0,
                expected_index: 1
            })
        );
    }
}
