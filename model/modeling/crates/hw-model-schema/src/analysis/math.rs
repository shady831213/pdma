use crate::ast::{ExpressionOp, LayoutExpr};
pub(super) fn binary(op: ExpressionOp, a: LayoutExpr, b: LayoutExpr) -> LayoutExpr {
    let (a, b) = (Box::new(a), Box::new(b));
    match op {
        ExpressionOp::Add => LayoutExpr::Add(a, b),
        ExpressionOp::Subtract => LayoutExpr::Subtract(a, b),
        ExpressionOp::Multiply => LayoutExpr::Multiply(a, b),
        ExpressionOp::AlignUp => LayoutExpr::AlignUp(a, b),
        ExpressionOp::Maximum => LayoutExpr::Maximum(a, b),
    }
}
