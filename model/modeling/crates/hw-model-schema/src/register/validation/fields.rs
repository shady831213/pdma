use core::fmt;

use crate::{RegisterDefinition, RegisterDefinitionError, RegisterFieldSpec};

pub(super) fn validate<I: Copy + Eq + fmt::Debug>(
    definition: RegisterDefinition<I>,
    fields: &[RegisterFieldSpec<I>],
    root: &'static str,
    path: &str,
    declaration_index: usize,
) -> Result<(), RegisterDefinitionError<I>> {
    let fields = ordered_fields(fields);
    validate_duplicates(&fields, root, path)?;
    validate_field_ranges(definition, &fields, root, path)?;
    validate_field_overlaps(&fields, root, path)?;
    validate_writable_coverage(definition, &fields, root, path, declaration_index)
}

fn validate_duplicates<I: Copy + Eq>(
    fields: &[(&RegisterFieldSpec<I>, usize)],
    root: &'static str,
    path: &str,
) -> Result<(), RegisterDefinitionError<I>> {
    for (position, field) in fields.iter().copied().enumerate() {
        if fields
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
    for (position, field) in fields.iter().copied().enumerate() {
        if fields
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
    fields: &[(&RegisterFieldSpec<I>, usize)],
    root: &'static str,
    path: &str,
) -> Result<(), RegisterDefinitionError<I>> {
    for &(field, index) in fields {
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
    for &(field, index) in fields {
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
    fields: &[(&RegisterFieldSpec<I>, usize)],
    root: &'static str,
    path: &str,
) -> Result<(), RegisterDefinitionError<I>> {
    for (position, (field, index)) in fields.iter().copied().enumerate() {
        for (previous, _) in fields.iter().copied().take(position) {
            if overlap(*previous, *field) {
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
    fields: &[(&RegisterFieldSpec<I>, usize)],
    root: &'static str,
    path: &str,
    declaration_index: usize,
) -> Result<(), RegisterDefinitionError<I>> {
    if fields.is_empty() {
        return Ok(());
    }
    let declared_field_mask = fields
        .iter()
        .fold(0, |mask, (field, _)| mask | field_mask(**field));
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

fn ordered_fields<I: Copy>(fields: &[RegisterFieldSpec<I>]) -> Vec<(&RegisterFieldSpec<I>, usize)> {
    let mut fields = fields
        .iter()
        .enumerate()
        .map(|(index, field)| (field, field.declared_at(index)))
        .collect::<Vec<_>>();
    fields.sort_by_key(|(_, index)| *index);
    fields
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
