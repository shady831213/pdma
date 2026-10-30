use super::super::{LayoutError, LayoutExpr, PlanValue};

#[derive(Default, Clone, Copy)]
pub(in super::super) struct Values {
    pub cursor: usize,
    pub alignment: usize,
    pub extent: usize,
    pub offset: usize,
    pub count: usize,
    pub stride: usize,
    pub end: usize,
}
impl Values {
    pub(in super::super) fn evaluate(
        &self,
        expression: &LayoutExpr,
        bindings: (&[&str], &[usize]),
    ) -> Result<usize, LayoutError> {
        expression.evaluate_with(bindings, |value| {
            Ok(match value {
                PlanValue::Cursor => self.cursor,
                PlanValue::ChildAlignment => self.alignment,
                PlanValue::ChildExtent => self.extent,
                PlanValue::Offset => self.offset,
                PlanValue::Count => self.count,
                PlanValue::Stride => self.stride,
                PlanValue::End => self.end,
            })
        })
    }
}
