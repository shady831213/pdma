use super::super::{ExpressionReference, PlacementProperty, Renderer};
use crate::graph::{
    ChildLayout, ExpressionOp, ExpressionVisitor, LayoutError, LayoutExpr, PlacementPlan, PlanValue,
};

pub(super) fn render<R: Renderer>(
    renderer: &mut R,
    expr: &LayoutExpr,
    placement: Option<&PlacementPlan>,
) -> Result<String, LayoutError> {
    expr.visit(&mut Format {
        renderer,
        placement,
    })
}
pub(in crate::export) fn reference<R: Renderer>(
    renderer: &mut R,
    reference: ExpressionReference<'_>,
) -> String {
    match reference {
        ExpressionReference::Parameter(name) => {
            format!("p_{}", renderer.expression_identifier(name))
        }
        ExpressionReference::Cursor => "cursor".into(),
        ExpressionReference::Placement { name, property } => format!(
            "result.{}_{}",
            renderer.expression_identifier(name),
            property.name()
        ),
        ExpressionReference::ChildExtent(name) => {
            format!("child_{}.extent", renderer.expression_identifier(name))
        }
        ExpressionReference::End(name) => format!("end_{}", renderer.expression_identifier(name)),
    }
}
struct Format<'a, R> {
    renderer: &'a mut R,
    placement: Option<&'a PlacementPlan>,
}
impl<R: Renderer> ExpressionVisitor for Format<'_, R> {
    type Output = String;
    fn constant(&mut self, value: usize) -> Result<String, LayoutError> {
        Ok(self.renderer.expression_constant(value))
    }
    fn parameter(&mut self, name: &'static str) -> Result<String, LayoutError> {
        Ok(self
            .renderer
            .expression_reference(ExpressionReference::Parameter(name)))
    }
    fn value(&mut self, value: PlanValue) -> Result<String, LayoutError> {
        if value == PlanValue::Cursor {
            return Ok(self
                .renderer
                .expression_reference(ExpressionReference::Cursor));
        }
        let placement = self.placement.ok_or(LayoutError::Geometry)?;
        let name = placement.reference.name();
        let symbol = match value {
            PlanValue::Cursor => ExpressionReference::Cursor,
            PlanValue::ChildAlignment => {
                return self.constant(match &placement.child {
                    ChildLayout::Fixed { alignment, .. } => *alignment,
                    ChildLayout::Call { plan, .. } => plan.alignment(),
                });
            }
            PlanValue::ChildExtent => match &placement.child {
                ChildLayout::Fixed { extent, .. } => return self.constant(*extent),
                ChildLayout::Call { .. } => ExpressionReference::ChildExtent(name),
            },
            PlanValue::Offset => ExpressionReference::Placement {
                name,
                property: PlacementProperty::Offset,
            },
            PlanValue::Count => ExpressionReference::Placement {
                name,
                property: PlacementProperty::Count,
            },
            PlanValue::Stride => ExpressionReference::Placement {
                name,
                property: PlacementProperty::Stride,
            },
            PlanValue::End => ExpressionReference::End(name),
        };
        Ok(self.renderer.expression_reference(symbol))
    }
    fn binary(
        &mut self,
        op: ExpressionOp,
        left: String,
        right: String,
    ) -> Result<String, LayoutError> {
        Ok(self.renderer.expression_binary(op, &left, &right))
    }
}

pub(in crate::export) fn binary<R: Renderer>(
    renderer: &mut R,
    op: ExpressionOp,
    left: &str,
    right: &str,
) -> String {
    match op {
        ExpressionOp::Add => format!("({left} + {right})"),
        ExpressionOp::Subtract => format!("({left} - {right})"),
        ExpressionOp::Multiply => format!("({left} * {right})"),
        ExpressionOp::Maximum => renderer.expression_maximum(left, right),
        ExpressionOp::AlignUp => {
            let one = renderer.expression_constant(1);
            format!("(({left} + {right} - {one}) & ~({right} - {one}))")
        }
    }
}
