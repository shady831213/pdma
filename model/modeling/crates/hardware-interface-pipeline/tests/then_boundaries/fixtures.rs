use hardware_interface_pipeline::{Stage, StepOutcome};

#[derive(Clone, Copy, Debug, Default)]
pub(super) enum Reply {
    #[default]
    Complete,
    Pending,
    Progressed,
    Failure,
    Missing,
}

impl Reply {
    const fn outcome(self, input: u32) -> StepOutcome<u32, u32, Fault, u32> {
        match self {
            Self::Complete => StepOutcome::Complete(Ok(input + 1)),
            Self::Pending => StepOutcome::Pending(input, input),
            Self::Progressed => StepOutcome::Progressed(input),
            Self::Failure => StepOutcome::Complete(Err(Fault::Failed)),
            Self::Missing => StepOutcome::Complete(Err(Fault::Missing)),
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub(super) enum Fault {
    Failed,
    Missing,
}

#[derive(Debug, Default)]
pub(super) struct Prepare {
    pub reply: Reply,
    pub calls: usize,
}

impl Stage<Vec<u32>> for Prepare {
    type Input = u32;
    type Output = u32;
    type Error = Fault;
    type Wait = u32;
    type State = u32;

    fn start(input: u32) -> Self::State {
        input
    }

    fn step(
        &mut self,
        _: &mut Vec<u32>,
        state: Self::State,
    ) -> StepOutcome<Self::State, u32, Fault, u32> {
        self.calls += 1;
        self.reply.outcome(state)
    }
}

#[derive(Debug, Default)]
pub(super) struct Effect {
    pub reply: Reply,
    pub calls: usize,
}

impl Stage<Vec<u32>> for Effect {
    type Input = u32;
    type Output = u32;
    type Error = Fault;
    type Wait = u32;
    type State = u32;

    fn start(input: u32) -> Self::State {
        input
    }

    fn step(
        &mut self,
        context: &mut Vec<u32>,
        state: Self::State,
    ) -> StepOutcome<Self::State, u32, Fault, u32> {
        self.calls += 1;
        context.push(state);
        self.reply.outcome(state)
    }
}
