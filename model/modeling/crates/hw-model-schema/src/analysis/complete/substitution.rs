//! Resolves parameter bindings and ordered geometry dependencies without optimizing.
use crate::ast::{ExpressionOp, ExpressionVisitor, LayoutError, LayoutExpr, PlanValue};
pub(super) struct Substitute<'a> {
    pub parameters: &'a [&'a str],
    pub arguments: &'a [LayoutExpr],
    pub values: &'a [(PlanValue, LayoutExpr)],
}
impl ExpressionVisitor for Substitute<'_> {
    type Output = LayoutExpr;
    fn constant(&mut self, n: usize) -> Result<LayoutExpr, LayoutError> {
        Ok(LayoutExpr::Constant(n))
    }
    fn parameter(&mut self, name: &'static str) -> Result<LayoutExpr, LayoutError> {
        self.parameters
            .iter()
            .position(|p| *p == name)
            .and_then(|i| self.arguments.get(i))
            .cloned()
            .ok_or(LayoutError::Parameter(name))
    }
    fn value(&mut self, key: PlanValue) -> Result<LayoutExpr, LayoutError> {
        self.values
            .iter()
            .find(|(k, _)| *k == key)
            .map(|(_, v)| v.clone())
            .ok_or(LayoutError::Geometry)
    }
    fn binary(
        &mut self,
        op: ExpressionOp,
        a: LayoutExpr,
        b: LayoutExpr,
    ) -> Result<LayoutExpr, LayoutError> {
        Ok(crate::analysis::math::binary(op, a, b))
    }
}
