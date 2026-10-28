#[derive(Debug)]
pub(super) enum Phase {
    Inputs,
    Plan,
    Outputs,
}
/// Stage-owned offers, completed values, active-child bits and output receipts.
#[derive(Debug)]
pub(super) struct FanState<S, V, T, O, E> {
    pub(super) inputs: Vec<Option<S>>,
    pub(super) input_active: Vec<bool>,
    pub(super) values: Vec<Option<V>>,
    pub(super) outputs: Vec<Option<T>>,
    pub(super) output_active: Vec<bool>,
    pub(super) receipts: Vec<(usize, O)>,
    pub(super) phase: Phase,
    pub(super) failure: Option<crate::BranchError<E>>,
}
impl<S, V, T, O, E> FanState<S, V, T, O, E> {
    pub(super) fn fail(&mut self, error: crate::BranchError<E>) {
        if self.failure.is_none() {
            self.failure = Some(error);
        }
        for (input, active) in self.inputs.iter_mut().zip(&mut self.input_active) {
            if input.take().is_some() {
                *active = false;
            }
        }
        for (output, active) in self.outputs.iter_mut().zip(&mut self.output_active) {
            if output.take().is_some() {
                *active = false;
            }
        }
        self.values.iter_mut().for_each(|value| *value = None);
        self.receipts.clear();
    }
    pub(super) fn prepare_outputs(&mut self, count: usize) {
        if self.outputs.is_empty() {
            self.outputs = (0..count).map(|_| None).collect();
            self.output_active = vec![false; count];
        }
    }
    pub(super) fn ready_phase(&self) -> Option<Phase> {
        let order = match self.phase {
            Phase::Inputs => [Phase::Inputs, Phase::Plan, Phase::Outputs],
            Phase::Plan => [Phase::Plan, Phase::Outputs, Phase::Inputs],
            Phase::Outputs => [Phase::Outputs, Phase::Inputs, Phase::Plan],
        };
        order.into_iter().find(|phase| match phase {
            Phase::Inputs => self.input_active.iter().any(|active| *active),
            Phase::Plan => self.values.iter().any(Option::is_some),
            Phase::Outputs => self.output_active.iter().any(|active| *active),
        })
    }

    pub(super) fn new(inputs: Vec<S>) -> Self {
        Self {
            values: (0..inputs.len()).map(|_| None).collect(),
            input_active: vec![true; inputs.len()],
            inputs: inputs.into_iter().map(Some).collect(),
            outputs: Vec::new(),
            output_active: Vec::new(),
            receipts: Vec::new(),
            phase: Phase::Inputs,
            failure: None,
        }
    }
}
