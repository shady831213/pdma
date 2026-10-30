use super::{LayoutError, LayoutExpr, PlanValue};

/// Binary operations whose child expressions are folded by the common traversal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExpressionOp {
    /// Checked sum.
    Add,
    /// Checked difference.
    Subtract,
    /// Checked product.
    Multiply,
    /// Round up to the right operand's alignment.
    AlignUp,
    /// Greater operand.
    Maximum,
}

/// Nonrecursive callbacks for evaluating or formatting one layout expression.
///
/// The common fold visits left then right before invoking `binary`. Unsupported
/// source expressions fail before invoking a callback for that node.
pub trait ExpressionVisitor {
    /// A folded value, such as a checked integer or target-language expression.
    type Output;
    /// Handles one nonnegative literal.
    fn constant(&mut self, value: usize) -> Result<Self::Output, LayoutError>;
    /// Handles one parameter in the enclosing map's scope.
    fn parameter(&mut self, name: &'static str) -> Result<Self::Output, LayoutError>;
    /// Handles one child property or previous placement intermediate.
    fn value(&mut self, value: PlanValue) -> Result<Self::Output, LayoutError>;
    /// Combines already folded children without traversing their source expressions.
    fn binary(
        &mut self,
        op: ExpressionOp,
        left: Self::Output,
        right: Self::Output,
    ) -> Result<Self::Output, LayoutError>;
}
impl LayoutExpr {
    /// Folds this expression in left-to-right postorder using nonrecursive callbacks.
    ///
    /// # Errors
    /// Propagates the first callback error or an unsupported source expression.
    pub fn visit<V: ExpressionVisitor>(&self, visitor: &mut V) -> Result<V::Output, LayoutError> {
        let mut binary = |op, left: &Self, right: &Self| {
            let left = left.visit(visitor)?;
            let right = right.visit(visitor)?;
            visitor.binary(op, left, right)
        };
        match self {
            Self::Constant(value) => visitor.constant(*value),
            Self::Parameter(name) => visitor.parameter(name),
            Self::Value(value) => visitor.value(*value),
            Self::Add(left, right) => binary(ExpressionOp::Add, left, right),
            Self::Subtract(left, right) => binary(ExpressionOp::Subtract, left, right),
            Self::Multiply(left, right) => binary(ExpressionOp::Multiply, left, right),
            Self::AlignUp(left, right) => binary(ExpressionOp::AlignUp, left, right),
            Self::Maximum(left, right) => binary(ExpressionOp::Maximum, left, right),
            Self::Unsupported(source) => Err(LayoutError::Unsupported(source)),
        }
    }
}
