use super::super::LayoutError;
use super::{ChildLayout, LayoutExpr, MapLayoutPlan};

impl MapLayoutPlan {
    /// Evaluates this map into constant geometry while retaining child identities.
    ///
    /// # Errors
    /// Rejects invalid arguments or instance geometry before returning a closed plan.
    pub fn bind(&self, arguments: &[usize]) -> Result<Self, LayoutError> {
        let (geometry, evaluated) = self.evaluate_values(arguments)?;
        let mut bound = self.clone();
        for ((placement, values), resolved) in bound
            .placements
            .iter_mut()
            .zip(evaluated)
            .zip(geometry.placements)
        {
            for (expression, value) in [
                (&mut placement.offset, values.offset),
                (&mut placement.count, values.count),
                (&mut placement.stride, values.stride),
                (&mut placement.end, values.end),
                (&mut placement.cursor, values.cursor),
            ] {
                *expression = LayoutExpr::Constant(value);
            }
            placement.child = ChildLayout::Fixed {
                alignment: values.alignment,
                extent: values.extent,
            };
            placement.resolved_arguments = resolved.arguments;
        }
        bound.extent = LayoutExpr::Constant(geometry.extent);
        bound.parameters = &[];
        Ok(bound)
    }
}
