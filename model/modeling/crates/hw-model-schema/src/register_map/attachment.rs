//! Validates externally supplied placement metadata before topology construction.
use super::{RegisterMapError, ResolvedRegisterMap, ResolvedRegisterMapPlacement};
use crate::RegisterBankError;
use core::ops::Range;

impl<I: Copy, F: Copy + Eq + core::fmt::Debug> ResolvedRegisterMap<I, F> {
    pub(super) fn validate_placements(
        &self,
        nodes: &[ResolvedRegisterMapPlacement<I>],
    ) -> Result<(), RegisterMapError<I, F>> {
        let mut preceding: Option<&ResolvedRegisterMapPlacement<I>> = None;
        let order_error = |ancestor: &str, descendant: &str| RegisterMapError::PlacementOrder {
            root: self.root,
            path: descendant.to_owned(),
            ancestor: ancestor.to_owned(),
        };
        for (index, node) in nodes.iter().enumerate() {
            let failure = |source| RegisterMapError::PlacementGeometry {
                root: self.root,
                path: node.path().to_owned(),
                source,
            };
            let end = placement_end(node).map_err(failure)?;
            if end > self.size || !is_child(self.root, node.path()) {
                return Err(failure(RegisterBankError::Extent));
            }
            let occupied = node.placement().offset()..end;
            for previous in nodes.iter().take(index) {
                if previous.path() == node.path() {
                    return Err(failure(RegisterBankError::Overlap));
                }
                let previous_range =
                    previous.placement().offset()..placement_end(previous).map_err(failure)?;
                if let Some(parent) = containing_range(previous, node.path()) {
                    require_containment(parent.map_err(failure)?, &occupied).map_err(failure)?;
                    if preceding.is_some_and(|last| {
                        last.path() != previous.path() && !is_child(previous.path(), last.path())
                    }) {
                        return Err(order_error(previous.path(), node.path()));
                    }
                } else if let Some(parent) = containing_range(node, previous.path()) {
                    require_containment(parent.map_err(failure)?, &previous_range)
                        .map_err(failure)?;
                    return Err(order_error(node.path(), previous.path()));
                } else if overlaps(&occupied, &previous_range) {
                    return Err(failure(RegisterBankError::Overlap));
                }
            }
            for leaf in &self.registers {
                let leaf_range = leaf.offset()
                    ..leaf
                        .offset()
                        .checked_add(leaf.definition().width())
                        .ok_or_else(|| failure(RegisterBankError::Overflow))?;
                if let Some(parent) = containing_range(node, leaf.path()) {
                    require_containment(parent.map_err(failure)?, &leaf_range).map_err(failure)?;
                } else if overlaps(&occupied, &leaf_range) {
                    return Err(failure(RegisterBankError::Overlap));
                }
            }
            preceding = Some(node);
        }
        Ok(())
    }
}

fn placement_end<I: Copy>(
    node: &ResolvedRegisterMapPlacement<I>,
) -> Result<usize, RegisterBankError> {
    let placement = node.placement();
    if placement.count() == 0 {
        return Err(RegisterBankError::ZeroCount);
    }
    let alignment = placement.alignment();
    if !alignment.is_power_of_two() || !placement.offset().is_multiple_of(alignment) {
        return Err(RegisterBankError::Alignment);
    }
    if (placement.is_bank() && placement.extent() == 0)
        || (!placement.is_bank() && placement.count() != 1)
    {
        return Err(RegisterBankError::Extent);
    }
    if placement.is_bank() {
        if !placement.stride().is_multiple_of(alignment) {
            return Err(RegisterBankError::Alignment);
        }
        if placement.stride() < placement.extent() {
            return Err(RegisterBankError::Overlap);
        }
    }
    crate::checked_placement_end(
        placement.offset(),
        placement.count(),
        placement.stride(),
        placement.extent(),
    )
    .ok_or(RegisterBankError::Overflow)
}

fn is_child(parent: &str, path: &str) -> bool {
    path.strip_prefix(parent)
        .is_some_and(|tail| tail.starts_with('.') || tail.starts_with('['))
}

fn containing_range<I: Copy>(
    node: &ResolvedRegisterMapPlacement<I>,
    path: &str,
) -> Option<Result<Range<usize>, RegisterBankError>> {
    let suffix = path.strip_prefix(node.path())?;
    let placement = node.placement();
    if placement.is_bank() {
        let suffix = suffix.strip_prefix('[')?;
        let Some((index, tail)) = suffix.split_once(']') else {
            return Some(Err(RegisterBankError::Extent));
        };
        if !tail.is_empty() && !tail.starts_with('.') {
            return None;
        }
        let Ok(index) = index.parse::<usize>() else {
            return Some(Err(RegisterBankError::Extent));
        };
        if index >= placement.count() {
            return Some(Err(RegisterBankError::Extent));
        }
        Some(
            crate::checked_bank_offset(placement.offset(), index, placement.stride())
                .and_then(|start| start.checked_add(placement.extent()).map(|end| start..end))
                .ok_or(RegisterBankError::Overflow),
        )
    } else if suffix.is_empty() || suffix.starts_with('.') {
        Some(
            placement
                .offset()
                .checked_add(placement.extent())
                .map(|end| placement.offset()..end)
                .ok_or(RegisterBankError::Overflow),
        )
    } else {
        None
    }
}

const fn require_containment(
    parent: Range<usize>,
    child: &Range<usize>,
) -> Result<(), RegisterBankError> {
    if child.start < parent.start || child.end > parent.end {
        Err(RegisterBankError::Extent)
    } else {
        Ok(())
    }
}

const fn overlaps(first: &Range<usize>, second: &Range<usize>) -> bool {
    if first.start == first.end || second.start == second.end {
        return false;
    }
    first.start < second.end && second.start < first.end
}
