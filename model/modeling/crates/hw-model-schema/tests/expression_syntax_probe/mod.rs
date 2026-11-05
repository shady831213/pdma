use super::*;
struct PrefixSyntax;
impl ExpressionSyntax for PrefixSyntax {
    fn binary(
        &mut self,
        op: hw_model_schema::ast::ExpressionOp,
        left: &str,
        right: &str,
    ) -> String {
        format!("({op:?} {left} {right})")
    }
    fn parameter(&mut self, name: &str) -> String {
        format!("Parameter({name:?})")
    }
}
#[test]
fn custom_syntax_uses_shared_recursive_expression_formatting() {
    // Given a source expression independent of any backend rendering protocol.
    let expression = LayoutExpr::Add(
        Box::new(LayoutExpr::Parameter("n")),
        Box::new(LayoutExpr::Constant(4)),
    );
    // When the shared expression fold applies custom operator and reference spelling.
    let text = format_expression(&expression, &mut PrefixSyntax).expect("complete formula");
    // Then syntax reflects both folded operands exactly.
    assert_eq!(text, "(Add Parameter(\"n\") 4)");
}
