//! One public expression fold owns recursion for numeric and text consumers.
use hw_model_schema::ast::{ExpressionOp, ExpressionVisitor, LayoutError, LayoutExpr, PlanValue};

#[derive(Default)]
struct Trace {
    events: Vec<String>,
}
impl ExpressionVisitor for Trace {
    type Output = String;
    fn constant(&mut self, value: usize) -> Result<String, LayoutError> {
        self.events.push(format!("constant:{value}"));
        Ok(value.to_string())
    }
    fn parameter(&mut self, name: &'static str) -> Result<String, LayoutError> {
        self.events.push(format!("parameter:{name}"));
        if name == "reject" {
            Err(LayoutError::Parameter(name))
        } else {
            Ok(name.into())
        }
    }
    fn value(&mut self, value: PlanValue) -> Result<String, LayoutError> {
        self.events.push(format!("value:{value:?}"));
        Ok(format!("{value:?}"))
    }
    fn binary(
        &mut self,
        op: ExpressionOp,
        left: String,
        right: String,
    ) -> Result<String, LayoutError> {
        self.events.push(format!("binary:{op:?}"));
        Ok(format!("{op:?}({left},{right})"))
    }
}
#[test]
fn visitor_receives_folded_children_in_left_to_right_postorder() -> Result<(), LayoutError> {
    // Given: one expression combines a parameter, placement value and alignment.
    let expr = LayoutExpr::AlignUp(
        Box::new(LayoutExpr::Add(
            Box::new(LayoutExpr::Parameter("n")),
            Box::new(LayoutExpr::Value(PlanValue::Cursor)),
        )),
        Box::new(LayoutExpr::Constant(8)),
    );
    // When: a text consumer folds it and a numeric consumer supplies both scopes.
    let mut trace = Trace::default();
    let text = expr.visit(&mut trace)?;
    let value = expr.evaluate_with((&["n"], &[3]), |value| match value {
        PlanValue::Cursor => Ok(6),
        PlanValue::ChildAlignment
        | PlanValue::ChildExtent
        | PlanValue::Offset
        | PlanValue::Count
        | PlanValue::Stride
        | PlanValue::End => Err(LayoutError::Geometry),
    })?;
    // Then: callbacks see resolved children in deterministic order, and arithmetic agrees.
    assert_eq!(text, "AlignUp(Add(n,Cursor),8)");
    assert_eq!(
        trace.events,
        [
            "parameter:n",
            "value:Cursor",
            "binary:Add",
            "constant:8",
            "binary:AlignUp"
        ]
    );
    assert_eq!(value, 16);
    assert_eq!(expr.validate(&["n"]), Err(LayoutError::Geometry));
    assert_eq!(expr.evaluate(&["n"], &[3]), Err(LayoutError::Geometry));
    Ok(())
}

#[test]
fn numeric_fold_checks_every_operation() {
    use LayoutExpr::{Add, AlignUp, Constant, Maximum, Multiply, Subtract};
    // Given: each public binary operation, including its arithmetic boundaries.
    let literal = |value| Box::new(Constant(value));
    let cases = [
        (Add(literal(7), literal(3)), Ok(10)),
        (Subtract(literal(7), literal(3)), Ok(4)),
        (Multiply(literal(7), literal(3)), Ok(21)),
        (AlignUp(literal(9), literal(8)), Ok(16)),
        (Maximum(literal(9), literal(8)), Ok(9)),
        (
            Add(literal(usize::MAX), literal(1)),
            Err(LayoutError::Geometry),
        ),
        (Subtract(literal(0), literal(1)), Err(LayoutError::Geometry)),
        (
            Multiply(literal(usize::MAX), literal(2)),
            Err(LayoutError::Geometry),
        ),
        (
            AlignUp(literal(usize::MAX), literal(8)),
            Err(LayoutError::Geometry),
        ),
        (AlignUp(literal(1), literal(0)), Err(LayoutError::Geometry)),
        (AlignUp(literal(1), literal(3)), Err(LayoutError::Geometry)),
    ];
    // When evaluated through the common visitor, then overflow never wraps.
    for (expression, expected) in cases {
        assert_eq!(expression.evaluate(&[], &[]), expected);
    }
    assert_eq!(
        Constant(1).evaluate(&["n"], &[]),
        Err(LayoutError::Arguments)
    );
    assert_eq!(
        LayoutExpr::Parameter("missing").evaluate(&[], &[]),
        Err(LayoutError::Parameter("missing"))
    );
}

#[test]
fn first_fold_error_stops_callbacks_without_replacing_the_cause() {
    // Given: the left parameter callback fails before an unsupported right subtree.
    let expression = LayoutExpr::Add(
        Box::new(LayoutExpr::Parameter("reject")),
        Box::new(LayoutExpr::Unsupported("never()")),
    );
    let mut trace = Trace::default();
    // When folded, then the original callback error survives and the right side is untouched.
    assert_eq!(
        expression.visit(&mut trace),
        Err(LayoutError::Parameter("reject"))
    );
    assert_eq!(trace.events, ["parameter:reject"]);
    assert_eq!(
        LayoutExpr::Unsupported("custom()").visit(&mut Trace::default()),
        Err(LayoutError::Unsupported("custom()"))
    );
    assert_eq!(
        LayoutExpr::Value(PlanValue::Count)
            .evaluate_with((&[], &[]), |_| Err(LayoutError::Overlap)),
        Err(LayoutError::Overlap)
    );
}
