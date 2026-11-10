//! Conservative mathematical simplification, preserving checked arithmetic failures.
use super::Constraint;
use crate::ast::{ExpressionOp, ExpressionVisitor, LayoutError, LayoutExpr, PlanValue};
struct Simplify<'a>(&'a [&'a str]);
impl ExpressionVisitor for Simplify<'_> {
    type Output = LayoutExpr;
    fn constant(&mut self, n: usize) -> Result<LayoutExpr, LayoutError> {
        Ok(LayoutExpr::Constant(n))
    }
    fn parameter(&mut self, name: &'static str) -> Result<LayoutExpr, LayoutError> {
        if self.0.contains(&name) {
            Ok(LayoutExpr::Parameter(name))
        } else {
            Err(LayoutError::Parameter(name))
        }
    }
    fn value(&mut self, _: PlanValue) -> Result<LayoutExpr, LayoutError> {
        Err(LayoutError::Geometry)
    }
    fn binary(
        &mut self,
        op: ExpressionOp,
        a: LayoutExpr,
        b: LayoutExpr,
    ) -> Result<LayoutExpr, LayoutError> {
        use LayoutExpr::Constant;
        match (&a, &b, op) {
            (
                _,
                Constant(0),
                ExpressionOp::Add | ExpressionOp::Subtract | ExpressionOp::Maximum,
            )
            | (_, Constant(1), ExpressionOp::Multiply | ExpressionOp::AlignUp) => return Ok(a),
            (Constant(0), _, ExpressionOp::Add | ExpressionOp::Maximum)
            | (Constant(1), _, ExpressionOp::Multiply) => return Ok(b),
            _ => {}
        }
        let constant = matches!((&a, &b), (Constant(_), Constant(_)));
        let expression = super::math::binary(op, a, b);
        if constant {
            expression.evaluate(&[], &[]).map(Constant)
        } else {
            Ok(expression)
        }
    }
}
pub(super) fn run(units: &mut [super::Unit]) -> Result<(), LayoutError> {
    for unit in units {
        if let super::Layout::Map(layout) = &mut unit.body
            && let Some(plan) = &mut layout.plan
        {
            simplify_map(plan)?;
        }
    }
    Ok(())
}

fn simplify_map(layout: &mut super::CompleteMapLayout) -> Result<(), crate::ast::LayoutError> {
    let map = layout;
    let mut simplify = Simplify(map.parameters);
    map.extent = map.extent.visit(&mut simplify)?;
    for placement in &mut map.placements {
        if let Some(child) = &mut placement.referent_layout {
            simplify_map(child)?;
        }
        for argument in &mut placement.arguments {
            *argument = argument.visit(&mut simplify)?;
        }
        for expression in [
            &mut placement.child_extent,
            &mut placement.offset,
            &mut placement.count,
            &mut placement.stride,
            &mut placement.end,
            &mut placement.cursor,
        ] {
            *expression = expression.visit(&mut simplify)?;
        }
    }
    for constraint in &mut map.constraints {
        let expressions = match constraint {
            Constraint::Positive(a) | Constraint::Aligned(a, _) => vec![a],
            Constraint::Fits(a, b) => vec![a, b],
            Constraint::Disjoint(a, b, c, d) => vec![a, b, c, d],
        };
        for expression in expressions {
            *expression = expression.visit(&mut simplify)?;
        }
        constraint.check(|expression| {
            Ok(match expression {
                LayoutExpr::Constant(value) => Some(*value),
                LayoutExpr::Parameter(_)
                | LayoutExpr::Value(_)
                | LayoutExpr::Add(..)
                | LayoutExpr::Subtract(..)
                | LayoutExpr::Multiply(..)
                | LayoutExpr::AlignUp(..)
                | LayoutExpr::Maximum(..)
                | LayoutExpr::Unsupported(_) => None,
            })
        })?;
    }
    Ok(())
}
