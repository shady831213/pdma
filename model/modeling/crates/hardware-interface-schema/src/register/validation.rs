use core::fmt;

use super::{
    RegisterDefinition, RegisterDefinitionError, RegisterFieldSpec, RegisterValidationError,
};

pub(super) const fn validate_legacy(
    name: &'static str,
    width: usize,
    writable_mask: u64,
    reset_value: u64,
) -> Result<(), RegisterValidationError> {
    if !supported_width(width) {
        return Err(RegisterValidationError::InvalidWidth { name, width });
    }
    if !fits_width(writable_mask, width) {
        return Err(RegisterValidationError::WritableMaskOverflow {
            name,
            width,
            mask: writable_mask,
        });
    }
    if !fits_width(reset_value, width) {
        return Err(RegisterValidationError::ResetValueOverflow {
            name,
            width,
            value: reset_value,
        });
    }
    Ok(())
}

pub(super) fn validate_definition<I: Copy + Eq + fmt::Debug>(
    definition: RegisterDefinition<I>,
    root: &'static str,
    path: &str,
    declaration_index: usize,
) -> Result<(), RegisterDefinitionError<I>> {
    validate_shape(definition, root, path, declaration_index)?;
    validate_duplicates(definition, root, path)?;
    validate_field_ranges(definition, root, path)?;
    validate_field_overlaps(definition, root, path)?;
    validate_writable_coverage(definition, root, path, declaration_index)
}

fn validate_shape<I: Copy>(
    definition: RegisterDefinition<I>,
    root: &'static str,
    path: &str,
    declaration_index: usize,
) -> Result<(), RegisterDefinitionError<I>> {
    if !supported_width(definition.width()) {
        return Err(RegisterDefinitionError::InvalidWidth {
            root,
            path: path.into(),
            declaration_index,
            name: definition.name(),
            width: definition.width(),
        });
    }
    if definition.alignment() == 0 || !definition.alignment().is_power_of_two() {
        return Err(RegisterDefinitionError::InvalidAlignment {
            root,
            path: path.into(),
            declaration_index,
            name: definition.name(),
            alignment: definition.alignment(),
        });
    }
    if !fits_width(definition.writable_mask(), definition.width()) {
        return Err(RegisterDefinitionError::WritableMaskOverflow {
            root,
            path: path.into(),
            declaration_index,
            name: definition.name(),
            width: definition.width(),
            writable_mask: definition.writable_mask(),
        });
    }
    if !fits_width(definition.reset_value(), definition.width()) {
        return Err(RegisterDefinitionError::ResetValueOverflow {
            root,
            path: path.into(),
            declaration_index,
            name: definition.name(),
            width: definition.width(),
            reset_value: definition.reset_value(),
        });
    }
    Ok(())
}

fn validate_duplicates<I: Copy + Eq>(
    definition: RegisterDefinition<I>,
    root: &'static str,
    path: &str,
) -> Result<(), RegisterDefinitionError<I>> {
    for (position, field) in ordered_fields(definition).iter().copied().enumerate() {
        if ordered_fields(definition)
            .iter()
            .copied()
            .take(position)
            .any(|previous| previous.0.id() == field.0.id())
        {
            return Err(RegisterDefinitionError::DuplicateFieldId {
                root,
                path: format!("{path}.{}", field.0.name()),
                declaration_index: field.1,
                id: field.0.id(),
            });
        }
    }
    for (position, field) in ordered_fields(definition).iter().copied().enumerate() {
        if ordered_fields(definition)
            .iter()
            .copied()
            .take(position)
            .any(|previous| previous.0.name() == field.0.name())
        {
            return Err(RegisterDefinitionError::DuplicateFieldName {
                root,
                path: format!("{path}.{}", field.0.name()),
                declaration_index: field.1,
                name: field.0.name(),
            });
        }
    }
    Ok(())
}

fn validate_field_ranges<I: Copy + Eq + fmt::Debug>(
    definition: RegisterDefinition<I>,
    root: &'static str,
    path: &str,
) -> Result<(), RegisterDefinitionError<I>> {
    for (field, index) in ordered_fields(definition) {
        let field_path = format!("{path}.{}", field.name());
        if field.bit_width() == 0 || field.range().is_none() {
            return Err(RegisterDefinitionError::FieldBitArithmetic {
                root,
                path: field_path,
                declaration_index: index,
                id: field.id(),
                lsb: field.lsb(),
                bit_width: field.bit_width(),
            });
        }
    }
    for (field, index) in ordered_fields(definition) {
        let Some(range) = field.range() else {
            continue;
        };
        if range.end > definition.width() * 8 {
            return Err(RegisterDefinitionError::FieldOutOfBounds {
                root,
                path: format!("{path}.{}", field.name()),
                declaration_index: index,
                id: field.id(),
                lsb: field.lsb(),
                bit_width: field.bit_width(),
                register_bits: definition.width() * 8,
            });
        }
    }
    Ok(())
}

fn validate_field_overlaps<I: Copy + Eq + fmt::Debug>(
    definition: RegisterDefinition<I>,
    root: &'static str,
    path: &str,
) -> Result<(), RegisterDefinitionError<I>> {
    let fields = ordered_fields(definition);
    for (position, (field, index)) in fields.iter().copied().enumerate() {
        for (previous, _) in fields.iter().copied().take(position) {
            if overlap(previous, field) {
                return Err(RegisterDefinitionError::FieldOverlap {
                    root,
                    path: path.into(),
                    declaration_index: index,
                    first_id: previous.id(),
                    second_id: field.id(),
                });
            }
        }
    }
    Ok(())
}

fn validate_writable_coverage<I: Copy>(
    definition: RegisterDefinition<I>,
    root: &'static str,
    path: &str,
    declaration_index: usize,
) -> Result<(), RegisterDefinitionError<I>> {
    if definition.fields().is_empty() {
        return Ok(());
    }
    let declared_field_mask = definition
        .fields()
        .iter()
        .copied()
        .fold(0, |mask, field| mask | field_mask(field));
    let outside_mask = definition.writable_mask() & !declared_field_mask;
    if outside_mask != 0 {
        return Err(RegisterDefinitionError::WritableMaskOutsideFields {
            root,
            path: path.into(),
            declaration_index,
            writable_mask: definition.writable_mask(),
            declared_field_mask,
            outside_mask,
        });
    }
    Ok(())
}

pub(super) fn ordered_fields<I: Copy>(
    definition: RegisterDefinition<I>,
) -> Vec<(RegisterFieldSpec<I>, usize)> {
    let mut fields = definition
        .fields()
        .iter()
        .copied()
        .enumerate()
        .map(|(index, field)| (field, field.declared_at(index)))
        .collect::<Vec<_>>();
    fields.sort_by_key(|(_, index)| *index);
    fields
}

pub(super) const fn fits_width(value: u64, width: usize) -> bool {
    width == 8 || (width < 8 && value < (1_u64 << (width * 8)))
}

const fn supported_width(width: usize) -> bool {
    matches!(width, 1 | 2 | 4 | 8)
}

const fn field_mask<I: Copy>(field: RegisterFieldSpec<I>) -> u64 {
    if field.bit_width() == 64 {
        u64::MAX
    } else {
        ((1_u64 << field.bit_width()) - 1) << field.lsb()
    }
}

const fn overlap<I: Copy>(first: RegisterFieldSpec<I>, second: RegisterFieldSpec<I>) -> bool {
    first.lsb() < second.lsb().saturating_add(second.bit_width())
        && second.lsb() < first.lsb().saturating_add(first.bit_width())
}
