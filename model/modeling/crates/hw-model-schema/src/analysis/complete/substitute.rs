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
        let mut result = Self::substitute(
            plan,
            &arguments,
            if bindings.is_some() {
                &[]
            } else {
                plan.parameters()
            },
        )?;
        if let Some(bindings) = bindings {
            result.parameters = &[];
            result.bindings = Some(bindings.to_vec());
        }
        Ok(result)
    }
    fn bind_child(
        plan: &MapLayoutPlan,
        args: &[LayoutExpr],
        scope: &'static [&'static str],
    ) -> Result<Self, LayoutError> {
        let mut child = Self::substitute(plan, args, scope)?;
        child.bindings = args
            .iter()
            .map(|v| match v.evaluate(&[], &[]) {
                Ok(value) => Ok(Some(value)),
                Err(LayoutError::Parameter(_)) => Ok(None),
                Err(error) => Err(error),
            })
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .collect();
        child.parameters = if child.bindings.is_some() { &[] } else { scope };
        Ok(child)
    }
    fn substitute(
        plan: &MapLayoutPlan,
        arguments: &[LayoutExpr],
        scope_parameters: &'static [&'static str],
    ) -> Result<Self, LayoutError> {
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
            let (child_alignment, child_extent, child_arguments, child_parameters, referent_layout) =
                match &placement.child {
                    ChildLayout::Fixed { alignment, extent } => (
                        *alignment,
                        LayoutExpr::Constant(*extent),
                        Vec::new(),
                        &[][..],
                        None,
                    ),
                    ChildLayout::Call { arguments, plan } => {
                        let args = arguments
                            .iter()
                            .map(|e| e.visit(&mut substitute))
                            .collect::<Result<Vec<_>, _>>()?;
                        let child = Self::bind_child(plan, &args, scope_parameters)?;
                        constraints.extend(child.constraints.clone());
                        (
                            child.alignment,
                            child.extent.clone(),
                            args,
                            plan.parameters(),
                            Some(Box::new(child)),
                        )
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
                referent_layout,
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
            parameters: scope_parameters,
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
