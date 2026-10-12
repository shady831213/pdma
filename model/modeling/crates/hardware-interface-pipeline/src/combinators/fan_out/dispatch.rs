use crate::{FanPlan, FanPolicy};
/// Routes a completed value to one destination.
pub trait Route<V, E> {
    /// Selects the destination without consuming the value.
    fn target(&mut self, value: &V) -> Result<usize, E>;
}
/// Singleton-input routing policy for a fan.
#[derive(Debug)]
pub struct Dispatch<R>(R);
impl<R> Dispatch<R> {
    /// Adapts an application's destination selection.
    pub const fn new(route: R) -> Self {
        Self(route)
    }
}
impl<V, E, R: Route<V, E>> FanPolicy<V, V, E> for Dispatch<R> {
    fn plan(&mut self, values: &[Option<V>], available: &[bool]) -> Result<FanPlan, E> {
        let Some(Some(value)) = values.first() else {
            return Ok(FanPlan::default());
        };
        let index = self.0.target(value)?;
        if available.get(index) == Some(&false) {
            return Ok(FanPlan::default());
        }
        Ok(FanPlan {
            inputs: vec![0],
            outputs: vec![index],
        })
    }
    fn route(&mut self, values: Vec<V>) -> Result<Vec<V>, E> {
        Ok(values)
    }
}
