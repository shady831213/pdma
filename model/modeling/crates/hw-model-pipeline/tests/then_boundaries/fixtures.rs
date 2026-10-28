use hw_model_pipeline::{RunError, Stage, StepOutcome, StepResult};

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
    pub pending: Option<u32>,
}

impl Stage<Vec<u32>> for Prepare {
    type Input = u32;
    type Output = u32;
    type Error = Fault;

    fn step(&mut self, _: &mut Vec<u32>, input: &mut Option<u32>) -> StepResult<u32, Fault> {
        if self.pending.is_none() {
            self.pending = input.take();
        }
        let Some(value) = self.pending else {
            return Ok(StepOutcome::InProgress);
        };
        self.calls += 1;
        {
            let result = self.reply.outcome(value);
            if matches!(result, Ok(StepOutcome::Complete(_))) {
                self.pending = None;
            }
            result
        }
    }
}

#[derive(Debug, Default)]
pub(super) struct Effect {
    pub reply: Reply,
    pub calls: usize,
    pub pending: Option<u32>,
}

impl Stage<Vec<u32>> for Effect {
    type Input = u32;
    type Output = u32;
    type Error = Fault;

    fn step(&mut self, context: &mut Vec<u32>, input: &mut Option<u32>) -> StepResult<u32, Fault> {
        if self.pending.is_none() {
            self.pending = input.take();
        }
        let Some(value) = self.pending else {
            return Ok(StepOutcome::InProgress);
        };
        self.calls += 1;
        context.push(value);
        {
            let result = self.reply.outcome(value);
            if matches!(result, Ok(StepOutcome::Complete(_))) {
                self.pending = None;
            }
            result
        }
    }
}
