//! Shared emission of analyzed layout computations.
use super::expression::{ExpressionSyntax, format_expression};
use crate::ast::analysis::program::{Call, LayoutProgram, Statement, Symbol};

/// Target syntax for individual statements; calculation order belongs to analysis.
pub trait ProgramSyntax: ExpressionSyntax {
    /// Target-owned emission failure.
    type Error;
    /// Spells a child layout call with arguments already formatted in caller scope.
    fn call(&mut self, call: &Call, arguments: &[String]) -> Result<(), Self::Error>;
    /// Spells an immutable local definition.
    fn define(&mut self, target: Symbol, value: &str) -> Result<(), Self::Error>;
    /// Spells an assignment to a declared local or result member.
    fn assign(&mut self, target: Symbol, value: &str) -> Result<(), Self::Error>;
}

/// Emits the analyzed body in order, formatting expressions through shared recursion.
///
/// # Errors
/// Stops at the first target syntax error.
pub fn emit_body<S: ProgramSyntax>(
    program: &LayoutProgram,
    syntax: &mut S,
) -> Result<(), S::Error> {
    for statement in &program.body {
        match statement {
            Statement::Call(call) => {
                let arguments = call
                    .arguments
                    .iter()
                    .map(|arg| format_expression(arg, syntax))
                    .collect::<Vec<_>>();
                syntax.call(call, &arguments)?;
            }
            Statement::Define { target, value } => {
                let value = format_expression(value, syntax);
                syntax.define(*target, &value)?;
            }
            Statement::Assign { target, value } => {
                let value = format_expression(value, syntax);
                syntax.assign(*target, &value)?;
            }
        }
    }
    Ok(())
}
