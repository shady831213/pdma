use crate::BranchError;
use std::collections::BTreeSet;

/// An ownership transfer from selected completed inputs to available outputs.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct FanPlan {
    /// Completed input positions consumed once, in this order.
    pub inputs: Vec<usize>,
    /// Available output positions receiving routed values, in this order.
    pub outputs: Vec<usize>,
}
/// Plans from completed values; it never receives or drives source hardware.
pub trait FanPolicy<V, I, E> {
    /// Selects values and idle output slots without consuming either.
    /// Called only when at least one completed value is buffered, even if every
    /// output is busy. Retained values may be offered again without new completions.
    fn plan(&mut self, values: &[Option<V>], available: &[bool]) -> Result<FanPlan, E>;
    /// Transforms the validated selection into one owned input per selected output.
    /// Sharing or cloning belongs explicitly to the policy.
    fn route(&mut self, values: Vec<V>) -> Result<Vec<I>, E>;
}
impl FanPlan {
    pub(super) fn validate<V, E>(
        self,
        values: &[Option<V>],
        available: &[bool],
    ) -> Result<Self, BranchError<E>> {
        validate_indices(&self.inputs, values.len(), |i| {
            values.get(i).is_some_and(Option::is_some)
        })?;
        validate_indices(&self.outputs, available.len(), |i| {
            available.get(i).copied().unwrap_or(false)
        })?;
        if self.inputs.is_empty() && !self.outputs.is_empty() {
            return Err(BranchError::EmptyInput);
        }
        Ok(self)
    }
}
fn validate_indices<E>(
    indices: &[usize],
    ports: usize,
    available: impl Fn(usize) -> bool,
) -> Result<(), BranchError<E>> {
    let mut seen = BTreeSet::new();
    for &index in indices {
        if index >= ports {
            return Err(BranchError::InvalidIndex { index, ports });
        }
        if !seen.insert(index) {
            return Err(BranchError::DuplicateIndex { index });
        }
        if !available(index) {
            return Err(BranchError::Unavailable { index });
        }
    }
    Ok(())
}
