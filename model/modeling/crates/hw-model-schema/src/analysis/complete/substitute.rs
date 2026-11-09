use super::substitution::Substitute;
use super::{CompleteMapLayout, CompletePlacementLayout, Constraint};
use crate::ast::{ChildLayout, LayoutError, LayoutExpr, MapLayoutPlan, PlanValue};
impl CompleteMapLayout {
    pub(in crate::analysis) fn resolve(
        plan: &MapLayoutPlan,
        bindings: Option<&[usize]>,
    ) -> Result<Self, LayoutError> {
        let arguments: Vec<_> = match bindings {
            Some(values) => {
                plan.evaluate(values)?;
                values.iter().copied().map(LayoutExpr::Constant).collect()
            }
            None => plan
                .parameters()
                .iter()
                .map(|name| LayoutExpr::Parameter(name))
                .collect(),
        };
        let mut result = Self::substitute(plan, &arguments)?;
        if let Some(bindings) = bindings {
            result.parameters = &[];
            result.bindings = Some(bindings.to_vec());
        }
        Ok(result)
    }
    fn substitute(plan: &MapLayoutPlan, arguments: &[LayoutExpr]) -> Result<Self, LayoutError> {
        use PlanValue::{ChildAlignment, ChildExtent, Count, Cursor, End, Offset, Stride};
        let mut substitute = Substitute {
            parameters: plan.parameters(),
            arguments,
            values: &[],
        };
        let mut cursor = LayoutExpr::Constant(0);
        let mut placements: Vec<CompletePlacementLayout> = Vec::new();
        let mut constraints = Vec::new();
        for placement in plan.placements() {
            let (child_alignment, child_extent, child_arguments, child_parameters) =
                match &placement.child {
                    ChildLayout::Fixed { alignment, extent } => (
                        *alignment,
                        LayoutExpr::Constant(*extent),
                        Vec::new(),
                        &[][..],
                    ),
                    ChildLayout::Call { arguments, plan } => {
                        let args = arguments
                            .iter()
                            .map(|e| e.visit(&mut substitute))
                            .collect::<Result<Vec<_>, _>>()?;
                        let child = Self::substitute(plan, &args)?;
                        constraints.extend(child.constraints);
                        (child.alignment, child.extent, args, plan.parameters())
                    }
                };
            let mut values = vec![
                (Cursor, cursor.clone()),
                (ChildAlignment, LayoutExpr::Constant(child_alignment)),
                (ChildExtent, child_extent.clone()),
            ];
            let mut fold = |expression: &LayoutExpr, key| {
                let expression = expression.visit(&mut Substitute {
                    parameters: plan.parameters(),
                    arguments,
                    values: &values,
                })?;
                values.push((key, expression.clone()));
                Ok::<_, LayoutError>(expression)
            };
            let offset = fold(&placement.offset, Offset)?;
            let count = fold(&placement.count, Count)?;
            let stride = fold(&placement.stride, Stride)?;
            let end = fold(&placement.end, End)?;
            cursor = if matches!(placement.offset, LayoutExpr::AlignUp(_, _)) {
                // Positive child extents and counts make implicit placement monotone.
                end.clone()
            } else {
                placement.cursor.visit(&mut Substitute {
                    parameters: plan.parameters(),
                    arguments,
                    values: &values,
                })?
            };
            let complete = CompletePlacementLayout {
                source: placement.source.clone(),
                inline_map: match (&placement.source.body(), &placement.child) {
                    (crate::ast::PlacementBody::Inline(_), ChildLayout::Call { plan, .. }) => {
                        Some(Box::new(Self::resolve(plan, None)?))
                    }
                    (crate::ast::PlacementBody::Reference(_), _)
                    | (crate::ast::PlacementBody::Inline(_), ChildLayout::Fixed { .. }) => None,
                },
                child_definition: crate::traversal::NodeId::definition(
                    placement.child_definition.clone(),
                ),
                child_name: placement.child_name.clone(),
                bank: placement.bank,
                documentation: placement.documentation,
                arguments: child_arguments,
                child_parameters,
                child_alignment,
                child_extent,
                offset,
                count,
                stride,
                end,
                cursor: cursor.clone(),
            };
            record_constraints(&complete, &placements, &mut constraints);
            placements.push(complete);
        }
        let extent = plan.extent().visit(&mut Substitute {
            parameters: plan.parameters(),
            arguments,
            values: &[(Cursor, cursor.clone())],
        })?;
        constraints.push(Constraint::Fits(cursor, extent.clone()));
        Ok(Self {
            parameters: plan.parameters(),
            source_parameters: plan.parameters(),
            bindings: None,
            alignment: plan.alignment(),
            placements,
            extent,
            constraints,
        })
    }
}

fn record_constraints(
    placement: &CompletePlacementLayout,
    previous: &[CompletePlacementLayout],
    constraints: &mut Vec<Constraint>,
) {
    constraints.extend([
        Constraint::Positive(placement.child_extent.clone()),
        Constraint::Positive(placement.count.clone()),
        Constraint::Aligned(placement.offset.clone(), placement.child_alignment),
    ]);
    for prior in previous {
        constraints.push(Constraint::Disjoint(
            placement.offset.clone(),
            placement.end.clone(),
            prior.offset.clone(),
            prior.end.clone(),
        ));
    }
}
