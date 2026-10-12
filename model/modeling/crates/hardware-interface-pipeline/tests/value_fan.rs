//! Independent input progress and retained values in a full fan.
use hardware_interface_pipeline::{
    Fan, FanPlan, FanPolicy, Operation, RunError, Stage, StepOutcome, StepResult,
};

mod value_fan {
    mod disposal;
    mod plans;
}

#[derive(Default)]
struct Context {
    released: bool,
    reads: Vec<usize>,
    writes: Vec<String>,
}
struct Source(usize);
impl Stage<Context> for Source {
    type Input = String;
    type Output = String;
    type Error = &'static str;
    type State = Option<String>;
    fn start(input: String) -> Self::State {
        Some(input)
    }
    fn step(
        &mut self,
        context: &mut Context,
        state: &mut Self::State,
    ) -> StepResult<String, Self::Error> {
        context.reads.push(self.0);
        if self.0 == 0 && !context.released {
            return Ok(StepOutcome::InProgress);
        }
        Ok(StepOutcome::Complete(Ok(state
            .take()
            .ok_or(RunError::Exhausted)?)))
    }
}
struct Sink;
impl Stage<Context> for Sink {
    type Input = String;
    type Output = usize;
    type Error = &'static str;
    type State = (Option<String>, bool);
    fn start(input: String) -> Self::State {
        (Some(input), false)
    }
    fn step(
        &mut self,
        context: &mut Context,
        state: &mut Self::State,
    ) -> StepResult<usize, Self::Error> {
        if !state.1 {
            state.1 = true;
            return Ok(StepOutcome::InProgress);
        }
        let value = state.0.take().ok_or(RunError::Exhausted)?;
        let len = value.len();
        context.writes.push(value);
        Ok(StepOutcome::Complete(Ok(len)))
    }
}
struct Select;
impl FanPolicy<String, String, &'static str> for Select {
    fn plan(
        &mut self,
        values: &[Option<String>],
        available: &[bool],
    ) -> Result<FanPlan, &'static str> {
        let input = values.iter().position(Option::is_some);
        let output = available.iter().position(|v| *v);
        Ok(match (input, output) {
            (Some(i), Some(o)) => FanPlan {
                inputs: vec![i],
                outputs: vec![o],
            },
            _ => FanPlan::default(),
        })
    }
    fn route(&mut self, values: Vec<String>) -> Result<Vec<String>, &'static str> {
        Ok(values)
    }
}

#[test]
fn incomplete_input_does_not_block_completed_values_or_outputs() {
    // Given: the first source stalls while the second has an owned value.
    let mut fan = Fan::new(vec![Source(0), Source(1)], Select, vec![Sink]);
    let mut operation = Operation::new(vec!["first".into(), "second".into()]);
    let mut context = Context::default();
    // When: several sweeps run before the first source is released.
    for _ in 0..9 {
        assert_eq!(
            operation.step(&mut fan, &mut context),
            Ok(StepOutcome::InProgress)
        );
    }
    // Then: the second value was consumed once without completing the whole batch.
    assert_eq!(context.writes, ["second"]);
    assert_eq!(context.reads.iter().filter(|&&id| id == 1).count(), 1);
    assert!(!operation.is_complete());
}

#[test]
fn completion_waits_for_retained_input_and_delivers_each_value_once() {
    // Given: a stalled source and another completed value share one output slot.
    let mut fan = Fan::new(vec![Source(0), Source(1)], Select, vec![Sink]);
    let first = "first".to_owned();
    let second = "second".to_owned();
    let expected = vec![(0, second.len()), (0, first.len())];
    let mut operation = Operation::new(vec![first, second]);
    let mut context = Context::default();
    for _ in 0..9 {
        assert_eq!(
            operation.step(&mut fan, &mut context),
            Ok(StepOutcome::InProgress)
        );
    }
    context.released = true;
    // When: the retained source becomes runnable and the batch drains.
    let result = (0..30).find_map(|_| match operation.step(&mut fan, &mut context) {
        Ok(StepOutcome::InProgress) => None,
        other => Some(other),
    });
    // Then: completion includes both values and repeated stepping is a call error.
    assert_eq!(result, Some(Ok(StepOutcome::Complete(Ok(expected)))));
    assert_eq!(context.writes, ["second", "first"]);
    assert_eq!(
        operation.step(&mut fan, &mut context),
        Err(RunError::Exhausted)
    );
}
