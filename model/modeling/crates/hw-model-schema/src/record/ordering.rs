use core::fmt;

use super::{FieldSpec, RecordPlacement, RecordSpec};

pub(super) fn ordered_fields<I: Copy + Eq + fmt::Debug>(
    spec: RecordSpec<I>,
) -> Vec<(FieldSpec<I>, usize)> {
    let mut fields = spec
        .fields()
        .iter()
        .copied()
        .enumerate()
        .map(|(index, field)| (field, field.declared_at(index)))
        .collect::<Vec<_>>();
    fields.sort_by_key(|(_, index)| *index);
    fields
}

pub(super) fn ordered_records<I: Copy + Eq + fmt::Debug>(
    spec: RecordSpec<I>,
) -> Vec<(RecordPlacement<I>, usize)> {
    let field_count = spec.fields().len();
    let mut records = spec
        .records()
        .iter()
        .copied()
        .enumerate()
        .map(|(index, record)| (record, record.declared_at(field_count + index)))
        .collect::<Vec<_>>();
    records.sort_by_key(|(_, index)| *index);
    records
}

pub(super) fn placement_path<I: Copy>(
    parent: &str,
    placement: RecordPlacement<I>,
    instance: usize,
) -> String {
    if placement.count() > 1 {
        format!("{parent}.{}[{instance}]", placement.name())
    } else {
        format!("{parent}.{}", placement.name())
    }
}

pub(super) const fn fields_overlap<I: Copy>(first: FieldSpec<I>, second: FieldSpec<I>) -> bool {
    let first_end = first.offset().saturating_add(first.width());
    let second_end = second.offset().saturating_add(second.width());
    first.offset() < second_end && second.offset() < first_end
}
