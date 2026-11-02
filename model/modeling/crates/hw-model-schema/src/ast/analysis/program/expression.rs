use super::LayoutProperty;
use crate::ast::{
    ChildLayout, ExpressionOp, ExpressionVisitor, LayoutError, LayoutExpr, PlacementPlan, PlanValue,
};

/// A resolved layout operand, independent of target syntax.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Symbol {
    /// Enclosing map parameter.
    Parameter(&'static str),
    /// Maximum occupied end of preceding placements.
    Cursor,
    /// Result property of a placement.
    Placement {
        /// Source placement name.
        name: &'static str,
        /// Selected property.
        property: LayoutProperty,
    },
    /// Extent returned by a child layout call.
    ChildExtent(&'static str),
    /// Exclusive end of one placement.
    End(&'static str),
    /// Enclosing map's final result extent.
    Extent,
}

/// An analyzed expression whose references no longer need layout interpretation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Expression {
    /// Nonnegative value.
    Literal(usize),
    /// Resolved local, parameter or result member.
    Reference(Symbol),
    /// Arithmetic with recursively analyzed operands.
    Binary {
        /// Arithmetic operation.
        op: ExpressionOp,
        /// Left operand.
        left: Box<Self>,
        /// Right operand.
        right: Box<Self>,
    },
}
impl Expression {
    pub(super) fn lower(
        expression: &LayoutExpr,
        placement: Option<&PlacementPlan>,
    ) -> Result<Self, LayoutError> {
        expression.visit(&mut Resolve(placement))
    }
}
struct Resolve<'a>(Option<&'a PlacementPlan>);
impl ExpressionVisitor for Resolve<'_> {
    type Output = Expression;
    fn constant(&mut self, value: usize) -> Result<Self::Output, LayoutError> {
        Ok(Expression::Literal(value))
    }
    fn parameter(&mut self, name: &'static str) -> Result<Self::Output, LayoutError> {
        Ok(Expression::Reference(Symbol::Parameter(name)))
    }
    fn value(&mut self, value: PlanValue) -> Result<Self::Output, LayoutError> {
        if value == PlanValue::Cursor {
            return Ok(Expression::Reference(Symbol::Cursor));
        }
        let placement = self.0.ok_or(LayoutError::Geometry)?;
        let name = placement.reference.name();
        let symbol = match value {
            PlanValue::Cursor => Symbol::Cursor,
            PlanValue::ChildAlignment => {
                return self.constant(match &placement.child {
                    ChildLayout::Fixed { alignment, .. } => *alignment,
                    ChildLayout::Call { plan, .. } => plan.alignment(),
                });
            }
            PlanValue::ChildExtent => match &placement.child {
                ChildLayout::Fixed { extent, .. } => return self.constant(*extent),
                ChildLayout::Call { .. } => Symbol::ChildExtent(name),
            },
            PlanValue::Offset => Symbol::Placement {
                name,
                property: LayoutProperty::Offset,
            },
            PlanValue::Count => Symbol::Placement {
                name,
                property: LayoutProperty::Count,
            },
            PlanValue::Stride => Symbol::Placement {
                name,
                property: LayoutProperty::Stride,
            },
            PlanValue::End => Symbol::End(name),
        };
        Ok(Expression::Reference(symbol))
    }
    fn binary(
        &mut self,
        op: ExpressionOp,
        left: Expression,
        right: Expression,
    ) -> Result<Expression, LayoutError> {
        Ok(Expression::Binary {
            op,
            left: Box::new(left),
            right: Box::new(right),
        })
    }
}
