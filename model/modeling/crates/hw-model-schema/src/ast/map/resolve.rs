use super::plan::EvaluationValues;
use super::{ChildLayout, LayoutError, MapDefinition, MapLayoutPlan};
use crate::ast::DefinitionId;

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
        self.layout_plan()?.evaluate(arguments)
    }
}
impl MapLayoutPlan {
    /// Evaluates the plan with checked arithmetic and validates every occupied envelope.
    pub fn evaluate(&self, arguments: &[usize]) -> Result<MapGeometry, LayoutError> {
        self.evaluate_values(arguments)
            .map(|(geometry, _)| geometry)
    }
    pub(super) fn evaluate_values(
        &self,
        arguments: &[usize],
    ) -> Result<(MapGeometry, Vec<EvaluationValues>), LayoutError> {
        if arguments.len() != self.parameters.len() {
            return Err(LayoutError::Arguments);
        }
        let mut values = EvaluationValues::default();
        let mut placements = Vec::new();
        let mut occupied = Vec::new();
        let mut evaluated = Vec::new();
        for planned in &self.placements {
            let (alignment, extent, bindings) = match &planned.child {
                ChildLayout::Fixed { alignment, extent } => {
                    (*alignment, *extent, planned.resolved_arguments.clone())
                }
                ChildLayout::Call {
                    arguments: expressions,
                    plan,
                } => {
                    let bindings = expressions
                        .iter()
                        .map(|expression| expression.evaluate(self.parameters, arguments))
                        .collect::<Result<Vec<_>, _>>()?;
                    let geometry = plan.evaluate(&bindings)?;
                    (geometry.alignment, geometry.extent, bindings)
                }
            };
            if extent == 0 {
                return Err(LayoutError::Geometry);
            }
            values.alignment = alignment;
            values.extent = extent;
            let inputs = (self.parameters, arguments);
            values.offset = values.evaluate(&planned.offset, inputs)?;
            values.count = values.evaluate(&planned.count, inputs)?;
            values.stride = values.evaluate(&planned.stride, inputs)?;
            if values.count == 0 || !values.offset.is_multiple_of(alignment) {
                return Err(LayoutError::Geometry);
            }
            values.end = values.evaluate(&planned.end, inputs)?;
            let EvaluationValues {
                offset,
                count,
                stride,
                end,
                ..
            } = values;
            if occupied
                .iter()
                .any(|(start, stop)| offset < *stop && end > *start)
            {
                return Err(LayoutError::Overlap);
            }
            occupied.push((offset, end));
            values.cursor = values.evaluate(&planned.cursor, inputs)?;
            evaluated.push(values);
            placements.push(PlacementGeometry {
                name: planned.source.name(),
                definition: planned.child_definition.clone(),
                arguments: bindings,
                offset,
                count,
                stride,
                extent,
            });
        }
        let extent = values.evaluate(&self.extent, (self.parameters, arguments))?;
        if values.cursor > extent {
            return Err(LayoutError::Geometry);
        }
        Ok((
            MapGeometry {
                alignment: self.alignment,
                extent,
                placements,
            },
            evaluated,
        ))
    }
}
