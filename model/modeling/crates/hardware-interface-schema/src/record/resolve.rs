use core::{fmt, ops::Range};

use super::ordering::{ordered_fields, ordered_records, placement_path};
use super::{
    FieldSpec, RecordGraphError, RecordPlacement, RecordSpec, ResolvedMember, ResolvedMemberKind,
    ResolvedRecord,
};

#[derive(Clone, Copy)]
enum DeclaredMember<I: 'static> {
    Field(FieldSpec<I>),
    Record(RecordPlacement<I>),
}

#[derive(Clone)]
struct Candidate<I: 'static> {
    member: ResolvedMember<I>,
    parent_path: String,
    is_record: bool,
}

#[derive(Clone, Copy)]
struct PlacementContext<'a, I: 'static> {
    parent: RecordSpec<I>,
    root: &'static str,
    path: &'a str,
    absolute_base: usize,
}

pub(super) fn resolve<I: Copy + Eq + fmt::Debug>(
    root: RecordSpec<I>,
) -> Result<ResolvedRecord<I>, RecordGraphError<I>> {
    super::validation::validate_graph(root)?;
    let mut candidates = Vec::new();
    collect_members(root, root.name(), root.name(), 0, &mut candidates)?;
    validate_overlaps(root.name(), &candidates)?;
    validate_paths(root.name(), &candidates)?;
    Ok(ResolvedRecord {
        root: root.name(),
        size: root.size(),
        members: candidates
            .into_iter()
            .map(|candidate| candidate.member)
            .collect(),
    })
}

fn collect_members<I: Copy + Eq + fmt::Debug>(
    spec: RecordSpec<I>,
    root: &'static str,
    path: &str,
    absolute_base: usize,
    candidates: &mut Vec<Candidate<I>>,
) -> Result<(), RecordGraphError<I>> {
    for (member, declaration_index) in ordered_members(spec) {
        match member {
            DeclaredMember::Field(field) => {
                let absolute_offset =
                    absolute_base.checked_add(field.offset()).ok_or_else(|| {
                        overflow_error(
                            root,
                            format!("{path}.{}", field.name()),
                            declaration_index,
                            field.offset(),
                            0,
                            0,
                        )
                    })?;
                candidates.push(Candidate {
                    member: ResolvedMember {
                        path: format!("{path}.{}", field.name()),
                        absolute_offset,
                        width: field.width(),
                        declaration_index,
                        kind: ResolvedMemberKind::Field {
                            id: field.id(),
                            definition: field.definition(),
                        },
                    },
                    parent_path: path.into(),
                    is_record: false,
                });
            }
            DeclaredMember::Record(placement) => {
                let context = PlacementContext {
                    parent: spec,
                    root,
                    path,
                    absolute_base,
                };
                collect_placement(context, placement, declaration_index, candidates)?;
            }
        }
    }
    Ok(())
}

fn collect_placement<I: Copy + Eq + fmt::Debug>(
    context: PlacementContext<'_, I>,
    placement: RecordPlacement<I>,
    declaration_index: usize,
    candidates: &mut Vec<Candidate<I>>,
) -> Result<(), RecordGraphError<I>> {
    for instance in 0..placement.count() {
        let member_path = placement_path(context.path, placement, instance);
        let stepped = instance
            .checked_mul(placement.stride())
            .and_then(|step| placement.offset().checked_add(step))
            .ok_or_else(|| {
                overflow_error(
                    context.root,
                    member_path.clone(),
                    declaration_index,
                    placement.offset(),
                    instance,
                    placement.stride(),
                )
            })?;
        let definition = placement.definition();
        let local_end = stepped.checked_add(definition.size()).ok_or_else(|| {
            overflow_error(
                context.root,
                member_path.clone(),
                declaration_index,
                placement.offset(),
                instance,
                placement.stride(),
            )
        })?;
        if local_end > context.parent.size() {
            return Err(RecordGraphError::PlacementOutOfBounds {
                root: context.root,
                path: member_path,
                declaration_index,
                offset: stepped,
                size: definition.size(),
                record_size: context.parent.size(),
            });
        }
        let absolute_offset = context.absolute_base.checked_add(stepped).ok_or_else(|| {
            overflow_error(
                context.root,
                member_path.clone(),
                declaration_index,
                placement.offset(),
                instance,
                placement.stride(),
            )
        })?;
        candidates.push(Candidate {
            member: ResolvedMember {
                path: member_path.clone(),
                absolute_offset,
                width: definition.size(),
                declaration_index,
                kind: ResolvedMemberKind::Record {
                    id: placement.id(),
                    definition,
                    instance,
                },
            },
            parent_path: context.path.into(),
            is_record: true,
        });
        collect_members(
            *definition,
            context.root,
            &member_path,
            absolute_offset,
            candidates,
        )?;
    }
    Ok(())
}

fn validate_overlaps<I: Copy>(
    root: &'static str,
    candidates: &[Candidate<I>],
) -> Result<(), RecordGraphError<I>> {
    for (position, candidate) in candidates.iter().enumerate() {
        for previous in candidates.iter().take(position) {
            if previous.parent_path == candidate.parent_path
                && (previous.is_record || candidate.is_record)
                && ranges_overlap(previous.member.range(), candidate.member.range())
            {
                return Err(RecordGraphError::MemberOverlap {
                    root,
                    path: candidate.parent_path.clone(),
                    declaration_index: candidate.member.declaration_index,
                    first_path: previous.member.path.clone(),
                    second_path: candidate.member.path.clone(),
                });
            }
        }
    }
    Ok(())
}

fn validate_paths<I: Copy>(
    root: &'static str,
    candidates: &[Candidate<I>],
) -> Result<(), RecordGraphError<I>> {
    for (position, candidate) in candidates.iter().enumerate() {
        if let Some(previous) = candidates
            .iter()
            .take(position)
            .find(|previous| previous.member.path == candidate.member.path)
        {
            return Err(RecordGraphError::ResolvedPathCollision {
                root,
                path: candidate.member.path.clone(),
                declaration_index: candidate.member.declaration_index,
                first_declaration_index: previous.member.declaration_index,
            });
        }
    }
    Ok(())
}

fn ordered_members<I: Copy + Eq + fmt::Debug>(
    spec: RecordSpec<I>,
) -> Vec<(DeclaredMember<I>, usize)> {
    let mut members = ordered_fields(spec)
        .into_iter()
        .map(|(field, index)| (DeclaredMember::Field(field), index))
        .chain(
            ordered_records(spec)
                .into_iter()
                .map(|(record, index)| (DeclaredMember::Record(record), index)),
        )
        .collect::<Vec<_>>();
    members.sort_by_key(|(_, index)| *index);
    members
}

const fn overflow_error<I>(
    root: &'static str,
    path: String,
    declaration_index: usize,
    offset: usize,
    instance: usize,
    stride: usize,
) -> RecordGraphError<I> {
    RecordGraphError::PlacementOffsetOverflow {
        root,
        path,
        declaration_index,
        offset,
        instance,
        stride,
    }
}

const fn ranges_overlap(first: Range<usize>, second: Range<usize>) -> bool {
    first.start < second.end && second.start < first.end
}
