use super::super::{LayoutError, MapPlacement};
use super::{ChildLayout, LayoutExpr, MapDefinition, MapLayoutPlan, PlacementPlan, PlanValue};
use crate::ast::{Definition, DefinitionBody, DefinitionId};

impl MapDefinition {
    /// Builds the common symbolic layout consumed by evaluators and exporters.
    pub fn layout_plan(&self) -> Result<MapLayoutPlan, LayoutError> {
        self.prepare(&mut Vec::new())
    }
    fn prepare(&self, ancestors: &mut Vec<DefinitionId>) -> Result<MapLayoutPlan, LayoutError> {
        use LayoutExpr::{Add, AlignUp, Constant, Maximum, Multiply, Subtract, Value};
        use PlanValue::{ChildAlignment, ChildExtent, Count, Cursor, End, Offset, Stride};
        if !self.alignment.is_power_of_two() {
            return Err(LayoutError::Geometry);
        }
        let placements = self
            .placements
            .iter()
            .map(|placement| {
                placement.count.validate(self.parameters)?;
                for argument in &placement.arguments {
                    argument.validate(self.parameters)?;
                }
                let (target, child) = self.child(placement, ancestors)?;
                Ok(PlacementPlan {
                    resolved_arguments: Vec::new(),
                    source: placement.clone(),
                    child_definition: target.id(),
                    child_name: target.name().to_owned(),
                    bank: placement.bank,
                    documentation: placement.documentation,
                    child,
                    offset: placement.offset.map_or_else(
                        || AlignUp(Box::new(Value(Cursor)), Box::new(Value(ChildAlignment))),
                        Constant,
                    ),
                    count: placement.count.clone(),
                    stride: AlignUp(
                        Box::new(Value(ChildExtent)),
                        Box::new(Value(ChildAlignment)),
                    ),
                    end: Add(
                        Box::new(Add(
                            Box::new(Value(Offset)),
                            Box::new(Multiply(
                                Box::new(Subtract(Box::new(Value(Count)), Box::new(Constant(1)))),
                                Box::new(Value(Stride)),
                            )),
                        )),
                        Box::new(Value(ChildExtent)),
                    ),
                    cursor: Maximum(Box::new(Value(Cursor)), Box::new(Value(End))),
                })
            })
            .collect::<Result<Vec<_>, LayoutError>>()?;
        let plan = MapLayoutPlan {
            parameters: self.parameters,
            alignment: self.alignment,
            placements,
            extent: self.extent.map_or(Value(Cursor), Constant),
        };
        if self.parameters.is_empty() {
            plan.evaluate(&[])?;
        }
        Ok(plan)
    }
}

impl MapDefinition {
    fn child(
        &self,
        placement: &MapPlacement,
        ancestors: &mut Vec<DefinitionId>,
    ) -> Result<(Definition, ChildLayout), LayoutError> {
        let target = placement.target();
        if ancestors.contains(&target.id()) {
            return Err(LayoutError::Cycle(target.id().path().to_owned()));
        }
        let child = match target.body() {
            DefinitionBody::Register(register) if placement.arguments.is_empty() => {
                ChildLayout::Fixed {
                    alignment: register.alignment(),
                    extent: register.width(),
                }
            }
            DefinitionBody::Opaque(spec) if placement.arguments.is_empty() => ChildLayout::Fixed {
                alignment: spec.alignment(),
                extent: spec.size(),
            },
            DefinitionBody::RegisterMap(map) => {
                if placement.arguments.len() != map.parameters.len() {
                    return Err(LayoutError::Arguments);
                }
                ancestors.push(target.id());
                let result = map.prepare(ancestors);
                ancestors.pop();
                ChildLayout::Call {
                    arguments: placement.arguments.clone(),
                    plan: Box::new(result?),
                }
            }
            DefinitionBody::Bits(_)
            | DefinitionBody::Record(_)
            | DefinitionBody::Union { .. }
            | DefinitionBody::Register(_)
            | DefinitionBody::Opaque(_) => return Err(LayoutError::Geometry),
        };
        let alignment = match &child {
            ChildLayout::Fixed { alignment, extent } => {
                if *extent == 0 {
                    return Err(LayoutError::Geometry);
                }
                *alignment
            }
            ChildLayout::Call { plan, .. } => plan.alignment,
        };
        if !alignment.is_power_of_two() || !self.alignment.is_multiple_of(alignment) {
            return Err(LayoutError::Geometry);
        }
        Ok((target, child))
    }
}
