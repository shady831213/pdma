use hardware_interface_pipeline::{
    Arbitrate, Arbitration, BranchError, FanIn, RunError, Stage, StepOutcome, StepResult,
};
use std::{cell::Cell, rc::Rc};

#[derive(Debug)]
pub(super) struct Sample {
    pub(super) id: usize,
    eligible: bool,
    drops: Rc<Cell<usize>>,
}
impl Drop for Sample {
    fn drop(&mut self) {
        self.drops.set(self.drops.get() + 1);
    }
}
#[derive(Default)]
pub(super) struct Context {
    pub(super) ready: Vec<bool>,
    pub(super) release: bool,
    pub(super) effects: Vec<usize>,
}
#[derive(Default)]
pub(super) struct Source(Option<(Sample, bool)>);
impl Stage<Context> for Source {
    type Input = (Sample, bool);
    type Output = Sample;
    type Error = ();
    fn step(
        &mut self,
        context: &mut Context,
        input: &mut Option<Self::Input>,
    ) -> StepResult<Sample, ()> {
        if self.0.is_none() {
            self.0 = input.take();
        }
        let Some((mut sample, gated)) = self.0.take() else {
            return Ok(StepOutcome::InProgress);
        };
        if gated && !context.release {
            self.0 = Some((sample, gated));
            return Ok(StepOutcome::InProgress);
        }
        sample.eligible = context.ready.get(sample.id).copied().unwrap_or(false);
        Ok(StepOutcome::Complete(Ok(sample)))
    }
}
pub(super) struct Select(pub(super) Rc<Cell<usize>>);
impl Arbitration<Sample, ()> for Select {
    fn select(&mut self, values: &[Option<Sample>]) -> Result<Option<usize>, ()> {
        self.0.set(self.0.get() + 1);
        Ok(values
            .iter()
            .position(|value| value.as_ref().is_some_and(|sample| sample.eligible)))
    }
}
pub(super) struct Output;
impl Stage<Context> for Output {
    type Input = Sample;
    type Output = Sample;
    type Error = ();
    fn step(
        &mut self,
        context: &mut Context,
        state: &mut Option<Self::Input>,
    ) -> StepResult<Sample, ()> {
        let sample = state.take().ok_or(RunError::Exhausted)?;
        context.effects.push(sample.id);
        Ok(StepOutcome::Complete(Ok(sample)))
    }
}
pub(super) type Hardware =
    FanIn<Source, Arbitrate<Select>, Output, (Sample, bool), Sample, Sample, Sample, ()>;
pub(super) type RoundResult = Result<Vec<(usize, Sample)>, BranchError<()>>;
pub(super) fn inputs(drops: &Rc<Cell<usize>>, count: usize) -> Vec<(Sample, bool)> {
    (0..count)
        .map(|id| {
            (
                Sample {
                    id,
                    eligible: false,
                    drops: Rc::clone(drops),
                },
                false,
            )
        })
        .collect()
}
pub(super) fn finish(
    hardware: &mut Hardware,
    state: &mut hardware_interface_pipeline::Operation<Hardware, Context>,
    context: &mut Context,
) -> Result<RoundResult, RunError> {
    for _ in 0..30 {
        match state.step(hardware, context)? {
            StepOutcome::InProgress => {}
            StepOutcome::Complete(result) => return Ok(result),
        }
    }
    Err(RunError::BudgetExhausted)
}
