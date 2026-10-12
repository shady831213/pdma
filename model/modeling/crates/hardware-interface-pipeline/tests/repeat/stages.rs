use std::ops::ControlFlow;

use hardware_interface_pipeline::{RunError, Stage, StepOutcome, StepResult};

pub(super) struct Visit {
    pub(super) calls: usize,
}

impl Stage<Vec<u32>> for Visit {
    type Input = u32;
    type Output = u32;
    type Error = String;
    type State = (u32, bool);

    fn start(input: Self::Input) -> Self::State {
        (input, false)
    }

    fn step(
        &mut self,
        context: &mut Vec<u32>,
        state: &mut Self::State,
    ) -> StepResult<Self::Output, Self::Error> {
        self.calls += 1;
        if state.1 {
            Ok(StepOutcome::Complete(Ok(state.0)))
        } else {
            context.push(state.0);
            state.1 = true;
            Ok(StepOutcome::InProgress)
        }
    }
}

pub(super) enum Finish {
    At(u32),
    Fail,
    Exhaust,
}

pub(super) struct Next {
    finish: Finish,
    pub(super) calls: usize,
}

impl Next {
    pub(super) const fn new(finish: Finish) -> Self {
        Self { finish, calls: 0 }
    }
}

impl Stage<Vec<u32>> for Next {
    type Input = u32;
    type Output = ControlFlow<String, u32>;
    type Error = String;
    type State = u32;

    fn start(input: Self::Input) -> Self::State {
        input
    }

    fn step(
        &mut self,
        _: &mut Vec<u32>,
        state: &mut Self::State,
    ) -> StepResult<Self::Output, Self::Error> {
        self.calls += 1;
        match self.finish {
            Finish::At(limit) if *state < limit => {
                Ok(StepOutcome::Complete(Ok(ControlFlow::Continue(*state + 1))))
            }
            Finish::At(_) => Ok(StepOutcome::Complete(Ok(ControlFlow::Break(format!(
                "finished {state}"
            ))))),
            Finish::Fail => Ok(StepOutcome::Complete(Err("child failure".to_owned()))),
            Finish::Exhaust => Err(RunError::Exhausted),
        }
    }
}
