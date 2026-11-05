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
    expression.visit(&mut Formatter(syntax))
}
struct Formatter<'a, S>(&'a mut S);
impl<S: ExpressionSyntax> crate::ast::ExpressionVisitor for Formatter<'_, S> {
    type Output = String;
    fn constant(&mut self, value: usize) -> Result<String, LayoutError> {
        Ok(self.0.literal(value))
    }
    fn parameter(&mut self, name: &'static str) -> Result<String, LayoutError> {
        Ok(self.0.parameter(name))
    }
    fn value(&mut self, _: crate::ast::PlanValue) -> Result<String, LayoutError> {
        Err(LayoutError::Geometry)
    }
    fn binary(
        &mut self,
        op: ExpressionOp,
        left: String,
        right: String,
    ) -> Result<String, LayoutError> {
        Ok(self.0.binary(op, &left, &right))
    }
}
