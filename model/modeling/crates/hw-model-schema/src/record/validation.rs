use core::fmt;

use super::ordering::{fields_overlap, ordered_fields, ordered_records, placement_path};
use super::{FieldSpec, RecordGraphError, RecordPlacement, RecordSpec, RecordValidationError};

#[derive(Clone, Copy)]
enum Sibling<I: 'static> {
    Field(FieldSpec<I>),
    Record(RecordPlacement<I>),
}

impl<I: Copy> Sibling<I> {
    const fn id(self) -> I {
        match self {
            Self::Field(field) => field.id(),
            Self::Record(record) => record.id(),
        }
    }

    const fn name(self) -> &'static str {
        match self {
            Self::Field(field) => field.name(),
            Self::Record(record) => record.name(),
        }
    }
}

pub(super) fn validate_flat<I: Copy + Eq + fmt::Debug>(
    spec: RecordSpec<I>,
) -> Result<(), RecordValidationError<I>> {
    for (index, field) in spec.fields().iter().copied().enumerate() {
        if spec
            .fields()
            .iter()
            .copied()
            .take(index)
            .any(|previous| previous.id() == field.id())
        {
            return Err(RecordValidationError::DuplicateFieldId { id: field.id() });
        }
    }
    for (index, field) in spec.fields().iter().copied().enumerate() {
        if spec
            .fields()
            .iter()
            .copied()
            .take(index)
            .any(|previous| previous.name() == field.name())
        {
            return Err(RecordValidationError::DuplicateFieldName { name: field.name() });
        }
    }
    for field in spec.fields().iter().copied() {
        if field.width() != field.scalar().width() {
            return Err(RecordValidationError::ScalarWidthMismatch {
                field_id: field.id(),
                scalar: field.scalar(),
                width: field.width(),
            });
        }
    }
    if spec.alignment() == 0 {
        return Err(RecordValidationError::AlignmentZero);
    }
    if !spec.alignment().is_power_of_two() {
        return Err(RecordValidationError::AlignmentNotPowerOfTwo {
            alignment: spec.alignment(),
        });
    }
    for field in spec.fields().iter().copied() {
        if field.range().is_none_or(|range| range.end > spec.size()) {
            return Err(RecordValidationError::FieldOutOfBounds {
                field_id: field.id(),
                offset: field.offset(),
                width: field.width(),
                record_size: spec.size(),
            });
        }
    }
    for (index, field) in spec.fields().iter().copied().enumerate() {
        for previous in spec.fields().iter().copied().take(index) {
            if fields_overlap(previous, field) {
                return Err(RecordValidationError::FieldOverlap {
                    first_id: previous.id(),
                    second_id: field.id(),
                });
            }
        }
    }
    Ok(())
}

pub(super) fn validate_graph<I: Copy + Eq + fmt::Debug>(
    root: RecordSpec<I>,
) -> Result<(), RecordGraphError<I>> {
    let mut active = vec![root];
    validate_definition(root, root.name(), root.name())?;
    validate_children(root, root.name(), root.name(), &mut active)
}

fn validate_children<I: Copy + Eq + fmt::Debug>(
    spec: RecordSpec<I>,
    root: &'static str,
    path: &str,
    active: &mut Vec<RecordSpec<I>>,
) -> Result<(), RecordGraphError<I>> {
    for (placement, declaration_index) in ordered_records(spec) {
        let definition = placement.definition();
        let child_path = placement_path(path, placement, 0);
        if active
            .iter()
            .copied()
            .any(|current| same_definition(current, *definition))
        {
            return Err(RecordGraphError::RecursiveRecord {
                root,
                path: child_path,
                declaration_index,
                definition: definition.name(),
            });
        }
        validate_definition(*definition, root, &child_path)?;
        active.push(*definition);
        validate_children(*definition, root, &child_path, active)?;
        active.pop();
    }
    Ok(())
}

fn same_definition<I: Copy + Eq + fmt::Debug>(first: RecordSpec<I>, second: RecordSpec<I>) -> bool {
    first.name() == second.name()
        && first.size() == second.size()
        && first.alignment() == second.alignment()
        && core::ptr::eq(first.fields(), second.fields())
        && core::ptr::eq(first.records(), second.records())
}

fn validate_definition<I: Copy + Eq + fmt::Debug>(
    spec: RecordSpec<I>,
    root: &'static str,
    path: &str,
) -> Result<(), RecordGraphError<I>> {
    let siblings = ordered_siblings(spec);
    for (position, (sibling, declaration_index)) in siblings.iter().copied().enumerate() {
        if siblings
            .iter()
            .copied()
            .take(position)
            .any(|(previous, _)| previous.id() == sibling.id())
        {
            return Err(RecordGraphError::DuplicateSiblingId {
                root,
                path: path.into(),
                declaration_index,
                id: sibling.id(),
            });
        }
    }
    for (position, (sibling, declaration_index)) in siblings.iter().copied().enumerate() {
        if siblings
            .iter()
            .copied()
            .take(position)
            .any(|(previous, _)| previous.name() == sibling.name())
        {
            return Err(RecordGraphError::DuplicateSiblingName {
                root,
                path: path.into(),
                declaration_index,
                name: sibling.name(),
            });
        }
    }
    validate_definition_shape(spec, root, path)
}

fn validate_definition_shape<I: Copy + Eq + fmt::Debug>(
    spec: RecordSpec<I>,
    root: &'static str,
    path: &str,
) -> Result<(), RecordGraphError<I>> {
    for (field, declaration_index) in ordered_fields(spec) {
        if field.width() != field.scalar().width() {
            return Err(RecordGraphError::ScalarWidthMismatch {
                root,
                path: format!("{path}.{}", field.name()),
                declaration_index,
                id: field.id(),
                scalar: field.scalar(),
                width: field.width(),
            });
        }
    }
    if spec.alignment() == 0 {
        return Err(RecordGraphError::AlignmentZero {
            root,
            path: path.into(),
            declaration_index: 0,
        });
    }
    if !spec.alignment().is_power_of_two() {
        return Err(RecordGraphError::AlignmentNotPowerOfTwo {
            root,
            path: path.into(),
            declaration_index: 0,
            alignment: spec.alignment(),
        });
    }
    validate_local_ranges(spec, root, path)
}

fn validate_local_ranges<I: Copy + Eq + fmt::Debug>(
    spec: RecordSpec<I>,
    root: &'static str,
    path: &str,
) -> Result<(), RecordGraphError<I>> {
    let fields = ordered_fields(spec);
    for (field, declaration_index) in fields.iter().copied() {
        if field.range().is_none_or(|range| range.end > spec.size()) {
            return Err(RecordGraphError::FieldOutOfBounds {
                root,
                path: format!("{path}.{}", field.name()),
                declaration_index,
                id: field.id(),
                offset: field.offset(),
                width: field.width(),
                record_size: spec.size(),
            });
        }
    }
    for (position, (field, declaration_index)) in fields.iter().copied().enumerate() {
        for (previous, _) in fields.iter().copied().take(position) {
            if fields_overlap(previous, field) {
                return Err(RecordGraphError::FieldOverlap {
                    root,
                    path: path.into(),
                    declaration_index,
                    first_id: previous.id(),
                    second_id: field.id(),
                });
            }
        }
    }
    Ok(())
}

fn ordered_siblings<I: Copy + Eq + fmt::Debug>(spec: RecordSpec<I>) -> Vec<(Sibling<I>, usize)> {
    let mut siblings = ordered_fields(spec)
        .into_iter()
        .map(|(field, index)| (Sibling::Field(field), index))
        .chain(
            ordered_records(spec)
                .into_iter()
                .map(|(record, index)| (Sibling::Record(record), index)),
        )
        .collect::<Vec<_>>();
    siblings.sort_by_key(|(_, index)| *index);
    siblings
}
