use crate::{FanPlan, FanPolicy};
/// Selects one completed value, without observing or driving source stages.
pub trait Arbitration<V, E> {
    /// Returns the selected buffered value, or no selection for this sweep.
    fn select(&mut self, values: &[Option<V>]) -> Result<Option<usize>, E>;
}
/// Singleton-output value-selection policy for a fan.
#[derive(Debug)]
pub struct Arbitrate<A>(A);
impl<A> Arbitrate<A> {
    /// Adapts an application value-selection policy.
    pub const fn new(policy: A) -> Self {
        Self(policy)
    }
}
impl<V, E, A: Arbitration<V, E>> FanPolicy<V, V, E> for Arbitrate<A> {
    fn plan(&mut self, values: &[Option<V>], available: &[bool]) -> Result<FanPlan, E> {
        if available.first() != Some(&true) {
            return Ok(FanPlan::default());
        }
        Ok(self
            .0
            .select(values)?
            .map_or_else(FanPlan::default, |index| FanPlan {
                inputs: vec![index],
                outputs: vec![0],
            }))
    }
    fn route(&mut self, values: Vec<V>) -> Result<Vec<V>, E> {
        Ok(values)
    }
}
