use hardware_interface_pipeline::{RunError, Stage, StepOutcome, StepResult};

#[derive(Clone, Copy, Debug, Default)]
pub(super) enum Reply {
    #[default]
    Complete,
    InProgress,
    Failure,
    Exhausted,
}

impl Reply {
    const fn outcome(self, input: u32) -> StepResult<u32, Fault> {
        match self {
            Self::Complete => Ok(StepOutcome::Complete(Ok(input + 1))),
            Self::InProgress => Ok(StepOutcome::InProgress),
            Self::Failure => Ok(StepOutcome::Complete(Err(Fault))),
            Self::Exhausted => Err(RunError::Exhausted),
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub(super) struct Fault;

#[derive(Debug, Default)]
pub(super) struct Prepare {
    pub reply: Reply,
    pub calls: usize,
}

impl Stage<Vec<u32>> for Prepare {
    type Input = u32;
    type Output = u32;
    type Error = Fault;
    type State = u32;

    fn start(input: u32) -> Self::State {
        input
    }

    fn step(&mut self, _: &mut Vec<u32>, state: &mut Self::State) -> StepResult<u32, Fault> {
        self.calls += 1;
        self.reply.outcome(*state)
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
    type State = u32;

    fn start(input: u32) -> Self::State {
        input
    }

    fn step(&mut self, context: &mut Vec<u32>, state: &mut Self::State) -> StepResult<u32, Fault> {
        self.calls += 1;
        context.push(*state);
        self.reply.outcome(*state)
    }
}
