use crate::ast::{ExpressionOp, LayoutError, LayoutExpr};

/// Nonrecursive target spelling for already analyzed expressions.
pub trait ExpressionSyntax: Sized {
    /// Formats a nonnegative literal.
    fn literal(&mut self, value: usize) -> String {
        value.to_string()
    }
    /// Escapes a source identifier.
    fn identifier(&mut self, value: &str) -> String {
        value.into()
    }
    /// Formats a maximum with folded operands.
    fn maximum(&mut self, left: &str, right: &str) -> String {
        format!("({left} > {right} ? {left} : {right})")
    }
    /// Formats an operator after both children have been folded.
    fn binary(&mut self, op: ExpressionOp, left: &str, right: &str) -> String {
        match op {
            ExpressionOp::Add => format!("({left} + {right})"),
            ExpressionOp::Subtract => format!("({left} - {right})"),
            ExpressionOp::Multiply => format!("({left} * {right})"),
            ExpressionOp::Maximum => self.maximum(left, right),
            ExpressionOp::AlignUp => {
                let one = self.literal(1);
                format!("(({left} + {right} - {one}) & ~({right} - {one}))")
            }
        }
    }
    /// Formats a schema parameter use.
    fn parameter(&mut self, name: &str) -> String {
        self.identifier(name)
    }
}

/// Folds semantic operands through target syntax without introducing target constructs.
pub fn format_expression<S: ExpressionSyntax>(
    expression: &LayoutExpr,
    syntax: &mut S,
) -> Result<String, LayoutError> {
    let mut binary = |op, left: &LayoutExpr, right: &LayoutExpr| {
        let left = format_expression(left, syntax)?;
        let right = format_expression(right, syntax)?;
        Ok(syntax.binary(op, &left, &right))
    };
    match expression {
        LayoutExpr::Constant(value) => Ok(syntax.literal(*value)),
        LayoutExpr::Parameter(name) => Ok(syntax.parameter(name)),
        LayoutExpr::Value(_) => Err(LayoutError::Geometry),
        LayoutExpr::Add(a, b) => binary(ExpressionOp::Add, a, b),
        LayoutExpr::Subtract(a, b) => binary(ExpressionOp::Subtract, a, b),
        LayoutExpr::Multiply(a, b) => binary(ExpressionOp::Multiply, a, b),
        LayoutExpr::Maximum(a, b) => binary(ExpressionOp::Maximum, a, b),
        LayoutExpr::AlignUp(a, b) => binary(ExpressionOp::AlignUp, a, b),
        LayoutExpr::Unsupported(source) => Err(LayoutError::Unsupported(source)),
    }
}
