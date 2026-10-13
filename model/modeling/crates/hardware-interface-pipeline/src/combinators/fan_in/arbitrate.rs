use crate::{FanPlan, FanPolicy};
/// Selects one completed value, without observing or driving source stages.
pub trait Arbitration<V, E> {
    /// Returns the selected buffered value, or no selection for this sweep.
    fn select(&mut self, values: &[Option<V>]) -> Result<Option<usize>, E>;
}
/// Singleton-output value-selection policy for a fan.
#[derive(Debug)]
pub struct Arbitrate<A> {
    policy: A,
    snapshots: bool,
    selected: bool,
}
impl<A> Arbitrate<A> {
    /// Adapts a selector, retaining unselected owned values for later selections.
    pub const fn new(policy: A) -> Self {
        Self {
            policy,
            snapshots: false,
            selected: false,
        }
    }
    /// Waits for every source, forwards one selected snapshot, and discards the rest.
    /// No selection discards the complete snapshot round without invoking the output.
    pub const fn snapshots(policy: A) -> Self {
        Self {
            policy,
            snapshots: true,
            selected: false,
        }
    }
}
impl<V, E, A: Arbitration<V, E>> FanPolicy<V, V, E> for Arbitrate<A> {
    fn plan(&mut self, values: &[Option<V>], available: &[bool]) -> Result<FanPlan, E> {
        if available.first() != Some(&true)
            || (self.snapshots && values.iter().any(Option::is_none))
        {
            return Ok(FanPlan::default());
        }
        let winner = self.policy.select(values)?;
        self.selected = winner.is_some();
        let mut inputs: Vec<_> = winner.into_iter().collect();
        if self.snapshots {
            inputs.extend((0..values.len()).filter(|index| Some(*index) != winner));
        }
        Ok(FanPlan {
            inputs,
            outputs: winner.map_or_else(Vec::new, |_| vec![0]),
        })
    }
    fn route(&mut self, values: Vec<V>) -> Result<Vec<V>, E> {
        Ok(values
            .into_iter()
            .take(usize::from(self.selected))
            .collect())
    }
}
