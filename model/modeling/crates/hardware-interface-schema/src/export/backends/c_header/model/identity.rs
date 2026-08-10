/// Failure while validating a caller-supplied typed C fragment model.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FragmentModelError {
    /// One identity occurs more than once.
    DuplicateIdentity {
        /// Index of the first occurrence.
        first_index: usize,
        /// Index of the duplicate occurrence.
        duplicate_index: usize,
    },
    /// One actual identity does not exist in the source schema.
    UnknownIdentity {
        /// Index of the unknown row.
        actual_index: usize,
    },
    /// One source-schema identity is absent from the model.
    MissingIdentity {
        /// Index of the missing source row.
        expected_index: usize,
    },
    /// The model contains all identities in a different order.
    ReorderedIdentity {
        /// Index where the unexpected identity occurs.
        actual_index: usize,
        /// Source-schema index for that identity.
        expected_index: usize,
    },
    /// One keyed row differs from its source-schema metadata.
    MismatchedRow {
        /// Index of the mismatched row.
        index: usize,
    },
}

pub(super) fn validate_rows<K, Row, Key>(
    expected: &[Row],
    actual: &[Row],
    key: Key,
) -> Result<(), FragmentModelError>
where
    K: Copy + Eq,
    Row: PartialEq,
    Key: Fn(&Row) -> K,
{
    for (duplicate_index, row) in actual.iter().enumerate() {
        if let Some(first_index) = actual
            .iter()
            .take(duplicate_index)
            .position(|previous| key(previous) == key(row))
        {
            return Err(FragmentModelError::DuplicateIdentity {
                first_index,
                duplicate_index,
            });
        }
    }
    for (actual_index, row) in actual.iter().enumerate() {
        if !expected.iter().any(|expected| key(expected) == key(row)) {
            return Err(FragmentModelError::UnknownIdentity { actual_index });
        }
    }
    for (expected_index, row) in expected.iter().enumerate() {
        if !actual.iter().any(|actual| key(actual) == key(row)) {
            return Err(FragmentModelError::MissingIdentity { expected_index });
        }
    }
    for (actual_index, (expected_row, actual_row)) in expected.iter().zip(actual).enumerate() {
        if key(expected_row) != key(actual_row) {
            let Some(expected_index) = expected
                .iter()
                .position(|candidate| key(candidate) == key(actual_row))
            else {
                return Err(FragmentModelError::UnknownIdentity { actual_index });
            };
            return Err(FragmentModelError::ReorderedIdentity {
                actual_index,
                expected_index,
            });
        }
        if expected_row != actual_row {
            return Err(FragmentModelError::MismatchedRow {
                index: actual_index,
            });
        }
    }
    Ok(())
}
