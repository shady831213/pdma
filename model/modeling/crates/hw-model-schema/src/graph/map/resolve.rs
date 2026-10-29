use super::{LayoutError, MapDefinition};
use crate::graph::{DefinitionBody, DefinitionId};

/// Concrete placement geometry linked to its reusable child definition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlacementGeometry {
    /// Placement name in its owner.
    pub name: &'static str,
    /// Referenced child definition.
    pub definition: DefinitionId,
    /// Child constructor bindings.
    pub arguments: Vec<usize>,
    /// Byte offset of the first child.
    pub offset: usize,
    /// Number of complete instances.
    pub count: usize,
    /// Distance between adjacent child instances.
    pub stride: usize,
    /// Occupied extent of one child.
    pub extent: usize,
}

/// Resolved instance geometry; separate configurations do not alter definition identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MapGeometry {
    /// Required placement alignment.
    pub alignment: usize,
    /// Occupied extent, including explicit holes.
    pub extent: usize,
    /// Child placements in declaration order.
    pub placements: Vec<PlacementGeometry>,
}

impl MapDefinition {
    /// Resolves the same aligned, noninterleaved bank geometry as generated constructors.
    pub fn resolve(&self, arguments: &[usize]) -> Result<MapGeometry, LayoutError> {
        self.resolve_inner(arguments, &mut Vec::new())
    }
    fn resolve_inner(
        &self,
        arguments: &[usize],
        ancestors: &mut Vec<DefinitionId>,
    ) -> Result<MapGeometry, LayoutError> {
        if arguments.len() != self.parameters.len() {
            return Err(LayoutError::Arguments);
        }
        if !self.alignment.is_power_of_two() {
            return Err(LayoutError::Geometry);
        }
        let mut cursor = 0;
        let mut placements = Vec::new();
        let mut occupied = Vec::new();
        for placement in &self.placements {
            let target = placement.reference.target();
            if ancestors.contains(&target.id()) {
                return Err(LayoutError::Cycle(target.id().path().to_owned()));
            }
            let bindings: Vec<_> = placement
                .arguments
                .iter()
                .map(|expression| expression.evaluate(self.parameters, arguments))
                .collect::<Result<_, _>>()?;
            let (alignment, extent) = match target.body() {
                DefinitionBody::Register(register) if bindings.is_empty() => {
                    (register.alignment(), register.width())
                }
                DefinitionBody::RegisterMap(map) => {
                    ancestors.push(target.id());
                    let geometry = map.resolve_inner(&bindings, ancestors)?;
                    ancestors.pop();
                    (geometry.alignment, geometry.extent)
                }
                DefinitionBody::Opaque(spec) if bindings.is_empty() => {
                    (spec.alignment(), spec.size())
                }
                DefinitionBody::Bits(_)
                | DefinitionBody::Record(_)
                | DefinitionBody::Union { .. }
                | DefinitionBody::Register(_)
                | DefinitionBody::Opaque(_) => return Err(LayoutError::Geometry),
            };
            if !alignment.is_power_of_two()
                || !self.alignment.is_multiple_of(alignment)
                || extent == 0
            {
                return Err(LayoutError::Geometry);
            }
            let offset = match placement.offset {
                Some(value) => value,
                None => crate::checked_align_up(cursor, alignment).ok_or(LayoutError::Geometry)?,
            };
            let count = placement.count.evaluate(self.parameters, arguments)?;
            let stride = crate::checked_align_up(extent, alignment).ok_or(LayoutError::Geometry)?;
            if count == 0 || !offset.is_multiple_of(alignment) {
                return Err(LayoutError::Geometry);
            }
            let end = crate::checked_placement_end(offset, count, stride, extent)
                .ok_or(LayoutError::Geometry)?;
            if occupied
                .iter()
                .any(|(start, stop)| offset < *stop && end > *start)
            {
                return Err(LayoutError::Overlap);
            }
            occupied.push((offset, end));
            cursor = cursor.max(end);
            placements.push(PlacementGeometry {
                name: placement.reference.name(),
                definition: target.id(),
                arguments: bindings,
                offset,
                count,
                stride,
                extent,
            });
        }
        let extent = self.extent.unwrap_or(cursor);
        if cursor > extent {
            return Err(LayoutError::Geometry);
        }
        Ok(MapGeometry {
            alignment: self.alignment,
            extent,
            placements,
        })
    }
}
