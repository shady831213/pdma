use crate::ast::{
    ExpressionOp,
    analysis::program::{Expression, Symbol},
};

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
    /// Formats a resolved symbol without consulting source layout rules.
    fn reference(&mut self, reference: Symbol) -> String {
        match reference {
            Symbol::Parameter(name) => format!("p_{}", self.identifier(name)),
            Symbol::Cursor => "cursor".into(),
            Symbol::Placement { name, property } => {
                format!("result.{}_{}", self.identifier(name), property.name())
            }
            Symbol::ChildExtent(name) => format!("child_{}.extent", self.identifier(name)),
            Symbol::End(name) => format!("end_{}", self.identifier(name)),
            Symbol::Extent => "result.extent".into(),
        }
    }
}

/// Folds analyzed operands through target syntax; no layout resolution remains.
pub fn format_expression<S: ExpressionSyntax>(expression: &Expression, syntax: &mut S) -> String {
    match expression {
        Expression::Literal(value) => syntax.literal(*value),
        Expression::Reference(symbol) => syntax.reference(*symbol),
        Expression::Binary { op, left, right } => {
            let left = format_expression(left, syntax);
            let right = format_expression(right, syntax);
            syntax.binary(*op, &left, &right)
        }
    }
}
