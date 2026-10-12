//! Value reduction and complete-plan validation.
use hardware_interface_pipeline::{
    Arbitrate, Arbitration, BranchError, FanIn, FanPlan, FanPolicy, RunError, Stage, StepOutcome,
    StepResult,
};
struct Number(u8);
impl Stage<Vec<u8>> for Number {
    type Input = ();
    type Output = u8;
    type Error = &'static str;
    type State = bool;
    fn start((): ()) -> bool {
        false
    }
    fn step(&mut self, context: &mut Vec<u8>, done: &mut bool) -> StepResult<u8, Self::Error> {
        if *done {
            return Err(RunError::Exhausted);
        }
        *done = true;
        context.push(self.0);
        Ok(StepOutcome::Complete(Ok(self.0)))
    }
}
struct Text;
impl Stage<Vec<u8>> for Text {
    type Input = String;
    type Output = String;
    type Error = &'static str;
    type State = Option<String>;
    fn start(input: String) -> Self::State {
        Some(input)
    }
    fn step(
        &mut self,
        context: &mut Vec<u8>,
        state: &mut Self::State,
    ) -> StepResult<String, Self::Error> {
        context.push(100);
        Ok(StepOutcome::Complete(Ok(state
            .take()
            .ok_or(RunError::Exhausted)?)))
    }
}
struct Sum(Vec<usize>);
impl FanPolicy<u8, String, &'static str> for Sum {
    fn plan(&mut self, values: &[Option<u8>], _: &[bool]) -> Result<FanPlan, &'static str> {
        if values.iter().all(Option::is_none) {
            return Ok(FanPlan::default());
        }
        Ok(FanPlan {
            inputs: self.0.clone(),
            outputs: vec![0],
        })
    }
    fn route(&mut self, values: Vec<u8>) -> Result<Vec<String>, &'static str> {
        Ok(vec![
            values
                .into_iter()
                .map(|v| v.to_string())
                .collect::<Vec<_>>()
                .join("+"),
        ])
    }
}
#[test]
fn completed_values_are_planned_before_any_output_effect() {
    // Given: a policy joins completed values in a custom order.
    let mut hardware = FanIn::new(vec![Number(3), Number(8)], Sum(vec![1, 0]), Text);
    let mut state = <FanIn<Number, Sum, Text> as Stage<Vec<u8>>>::start(vec![(); 2]);
    let mut context = Vec::new();
    // When: input collection and planning run in separate steps.
    assert_eq!(
        hardware.step(&mut context, &mut state),
        Ok(StepOutcome::InProgress)
    );
    assert_eq!(context, [3, 8]);
    assert_eq!(
        hardware.step(&mut context, &mut state),
        Ok(StepOutcome::InProgress)
    );
    assert_eq!(context, [3, 8]);
    let result = hardware.step(&mut context, &mut state);
    // Then: the output receives the owned transformed values in plan order.
    assert_eq!(
        result,
        Ok(StepOutcome::Complete(Ok(vec![(0, "8+3".into())])))
    );
    assert_eq!(context, [3, 8, 100]);
}
#[test]
fn invalid_input_plans_fail_before_any_output_effect() {
    // Given: malformed plans start with a valid completed input.
    for (indices, expected) in [
        (vec![0, 2], BranchError::InvalidIndex { index: 2, ports: 2 }),
        (vec![0, 0], BranchError::DuplicateIndex { index: 0 }),
    ] {
        let mut hardware = FanIn::new(vec![Number(3), Number(8)], Sum(indices), Text);
        let mut state = <FanIn<Number, Sum, Text> as Stage<Vec<u8>>>::start(vec![(); 2]);
        let mut context = Vec::new();
        assert_eq!(
            hardware.step(&mut context, &mut state),
            Ok(StepOutcome::InProgress)
        );
        // When: the plan is validated against buffered values.
        let result = hardware.step(&mut context, &mut state);
        // Then: invalid selection performs no destination effect and clears invocation state.
        assert_eq!(result, Ok(StepOutcome::Complete(Err(expected))));
        assert_eq!(context, [3, 8]);
        assert!(state.is_none());
    }
}
struct First;
impl Arbitration<u8, &'static str> for First {
    fn select(&mut self, values: &[Option<u8>]) -> Result<Option<usize>, &'static str> {
        Ok(values.iter().position(Option::is_some))
    }
}
struct Echo;
impl Stage<Vec<u8>> for Echo {
    type Input = u8;
    type Output = u8;
    type Error = &'static str;
    type State = Option<u8>;
    fn start(input: u8) -> Self::State {
        Some(input)
    }
    fn step(&mut self, _: &mut Vec<u8>, state: &mut Self::State) -> StepResult<u8, Self::Error> {
        Ok(StepOutcome::Complete(Ok(state
            .take()
            .ok_or(RunError::Exhausted)?)))
    }
}
#[test]
fn arbitration_retains_unselected_values_until_single_output_can_reuse() {
    // Given: two completed values share one output and a singleton selection policy.
    let mut hardware = FanIn::new(vec![Number(3), Number(8)], Arbitrate::new(First), Echo);
    let mut state = <FanIn<Number, Arbitrate<First>, Echo> as Stage<Vec<u8>>>::start(vec![(); 2]);
    let mut context = Vec::new();
    // When: the first output completes before the second buffered value is selected.
    for _ in 0..3 {
        assert_eq!(
            hardware.step(&mut context, &mut state),
            Ok(StepOutcome::InProgress)
        );
    }
    assert_eq!(context, [3, 8]);
    for _ in 0..2 {
        assert_eq!(
            hardware.step(&mut context, &mut state),
            Ok(StepOutcome::InProgress)
        );
    }
    let result = hardware.step(&mut context, &mut state);
    // Then: aggregate completion includes both values without rerunning source hardware.
    assert_eq!(result, Ok(StepOutcome::Complete(Ok(vec![(0, 3), (0, 8)]))));
    assert_eq!(context, [3, 8]);
}
#[test]
fn input_count_mismatch_fails_before_source_effects() {
    // Given: a batch omits an input required by the configured population.
    let mut hardware = FanIn::new(vec![Number(3)], Sum(vec![0]), Text);
    let mut state = <FanIn<Number, Sum, Text> as Stage<Vec<u8>>>::start(vec![]);
    let mut context = Vec::new();
    // When: the invocation is first stepped.
    let result = hardware.step(&mut context, &mut state);
    // Then: the malformed batch has no source or destination effects.
    assert_eq!(
        result,
        Ok(StepOutcome::Complete(Err(BranchError::InputCount {
            expected: 1,
            actual: 0
        })))
    );
    assert!(context.is_empty());
}
