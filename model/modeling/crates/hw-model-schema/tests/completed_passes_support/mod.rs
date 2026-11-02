use hw_model_schema::ast::{ExpressionOp, ExpressionVisitor, LayoutError, PlanValue};
/// Counts expression nodes, rejecting any unresolved geometry dependency.
#[derive(Debug)]
pub struct Count;
impl ExpressionVisitor for Count {
    type Output = usize;
    fn constant(&mut self, _: usize) -> Result<usize, LayoutError> {
        Ok(1)
    }
    fn parameter(&mut self, _: &'static str) -> Result<usize, LayoutError> {
        Ok(1)
    }
    fn value(&mut self, _: PlanValue) -> Result<usize, LayoutError> {
        Err(LayoutError::Geometry)
    }
    fn binary(&mut self, _: ExpressionOp, a: usize, b: usize) -> Result<usize, LayoutError> {
        Ok(a + b + 1)
    }
}
