#[derive(Debug)]
pub(super) enum Phase {
    Inputs,
    Plan,
    Outputs,
}
/// Per-invocation buffers: one state or completed value per input and one state per output.
#[derive(Debug)]
pub struct FanState<S, V, T, O> {
    pub(super) inputs: Vec<Option<S>>,
    pub(super) values: Vec<Option<V>>,
    pub(super) outputs: Vec<Option<T>>,
    pub(super) receipts: Vec<(usize, O)>,
    pub(super) phase: Phase,
}
impl<S, V, T, O> FanState<S, V, T, O> {
    pub(super) fn ready_phase(&self) -> Option<Phase> {
        let order = match self.phase {
            Phase::Inputs => [Phase::Inputs, Phase::Plan, Phase::Outputs],
            Phase::Plan => [Phase::Plan, Phase::Outputs, Phase::Inputs],
            Phase::Outputs => [Phase::Outputs, Phase::Inputs, Phase::Plan],
        };
        order.into_iter().find(|phase| match phase {
            Phase::Inputs => self.inputs.iter().any(Option::is_some),
            Phase::Plan => self.values.iter().any(Option::is_some),
            Phase::Outputs => self.outputs.iter().any(Option::is_some),
        })
    }

    pub(super) fn new(inputs: Vec<S>) -> Self {
        Self {
            values: (0..inputs.len()).map(|_| None).collect(),
            inputs: inputs.into_iter().map(Some).collect(),
            outputs: Vec::new(),
            receipts: Vec::new(),
            phase: Phase::Inputs,
        }
    }
}
