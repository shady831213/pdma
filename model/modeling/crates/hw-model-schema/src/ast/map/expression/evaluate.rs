use super::{ExpressionOp, ExpressionVisitor, LayoutError, LayoutExpr, PlanValue};

struct Evaluate<'a, F> {
    parameters: &'a [&'a str],
    arguments: &'a [usize],
    values: F,
}
impl<F: FnMut(PlanValue) -> Result<usize, LayoutError>> ExpressionVisitor for Evaluate<'_, F> {
    type Output = usize;
    fn constant(&mut self, value: usize) -> Result<usize, LayoutError> {
        Ok(value)
    }
    fn parameter(&mut self, name: &'static str) -> Result<usize, LayoutError> {
        self.parameters
            .iter()
            .position(|parameter| *parameter == name)
            .and_then(|index| self.arguments.get(index))
            .copied()
            .ok_or(LayoutError::Parameter(name))
    }
    fn value(&mut self, value: PlanValue) -> Result<usize, LayoutError> {
        (self.values)(value)
    }
    fn binary(
        &mut self,
        op: ExpressionOp,
        left: usize,
        right: usize,
    ) -> Result<usize, LayoutError> {
        match op {
            ExpressionOp::Add => left.checked_add(right),
            ExpressionOp::Subtract => left.checked_sub(right),
            ExpressionOp::Multiply => left.checked_mul(right),
            ExpressionOp::AlignUp => crate::checked_align_up(left, right),
            ExpressionOp::Maximum => Some(left.max(right)),
        }
        .ok_or(LayoutError::Geometry)
    }
}
struct Validate<'a>(&'a [&'a str]);
impl ExpressionVisitor for Validate<'_> {
    type Output = ();
    fn constant(&mut self, _value: usize) -> Result<(), LayoutError> {
        Ok(())
    }
    fn parameter(&mut self, name: &'static str) -> Result<(), LayoutError> {
        if self.0.contains(&name) {
            Ok(())
        } else {
            Err(LayoutError::Parameter(name))
        }
    }
    fn value(&mut self, _value: PlanValue) -> Result<(), LayoutError> {
        Err(LayoutError::Geometry)
    }
    fn binary(&mut self, _op: ExpressionOp, (): (), (): ()) -> Result<(), LayoutError> {
        Ok(())
    }
}
impl LayoutExpr {
    /// Evaluates constructor expressions; placement intermediates are outside this scope.
    pub fn evaluate(&self, parameters: &[&str], arguments: &[usize]) -> Result<usize, LayoutError> {
        self.evaluate_with((parameters, arguments), |_| Err(LayoutError::Geometry))
    }
    /// Evaluates with explicit parameter bindings and child/intermediate value resolution.
    ///
    /// # Errors
    /// Rejects arity mismatch, unknown parameters, unsupported expressions, arithmetic
    /// overflow/underflow, invalid alignment or a rejected intermediate value.
    pub fn evaluate_with(
        &self,
        bindings: (&[&str], &[usize]),
        values: impl FnMut(PlanValue) -> Result<usize, LayoutError>,
    ) -> Result<usize, LayoutError> {
        let (parameters, arguments) = bindings;
        if parameters.len() != arguments.len() {
            return Err(LayoutError::Arguments);
        }
        self.visit(&mut Evaluate {
            parameters,
            arguments,
            values,
        })
    }
    /// Checks constructor scope and portability without choosing instance values.
    pub fn validate(&self, parameters: &[&str]) -> Result<(), LayoutError> {
        self.visit(&mut Validate(parameters))
    }
}
