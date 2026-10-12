//! Application-defined fan-in behavior.

use hardware_interface_pipeline::{
    BranchError, Combine, FanIn, FanInState, Operation, Stage, StepOutcome,
};

struct Number(u8);

impl Stage<Vec<u8>> for Number {
    type Input = ();
    type Output = u8;
    type Error = &'static str;
    type Wait = String;
    type State = bool;

    fn start((): ()) -> bool {
        false
    }

    fn step(
        &mut self,
        context: &mut Vec<u8>,
        done: &mut bool,
    ) -> StepOutcome<u8, Self::Error, String> {
        if *done {
            return StepOutcome::Exhausted;
        }
        *done = true;
        context.push(self.0);
        StepOutcome::Complete(Ok(self.0))
    }
}

struct Sum(Vec<usize>);

impl Combine<Vec<u8>, Number> for Sum {
    type Output = String;
    type State = ();

    fn start() {}

    fn plan(
        &mut self,
        _: &Vec<u8>,
        _: &[Number],
        (): &mut (),
    ) -> StepOutcome<Vec<usize>, BranchError<&'static str>, String> {
        StepOutcome::Complete(Ok(std::mem::take(&mut self.0)))
    }

    fn combine(&mut self, (): (), values: Vec<u8>) -> Result<String, BranchError<&'static str>> {
        Ok(values.into_iter().map(u16::from).sum::<u16>().to_string())
    }
}

#[test]
fn planning_finishes_before_any_child_effect() {
    // Given: a valid plan selects an immediately completing child.
    let mut hardware = FanIn::new(vec![Number(3)], Sum(vec![0]));
    let mut context = Vec::new();
    let mut operation = Operation::new(());
    // When: the operation performs its planning step.
    let outcome = operation.step(&mut hardware, &mut context);
    // Then: execution stops at the boundary before entering the child.
    assert_eq!(outcome, StepOutcome::Progressed);
    assert!(context.is_empty());
}

#[test]
fn all_inputs_reduce_to_application_output_in_planned_order() {
    // Given: an application plans all inputs in a custom order and outputs text.
    let mut hardware = FanIn::new(vec![Number(3), Number(8)], Sum(vec![1, 0]));
    let mut context = Vec::new();
    let mut operation = Operation::new(());
    // When: planning, both children, and reduction run in separate steps.
    assert_eq!(
        operation.step(&mut hardware, &mut context),
        StepOutcome::Progressed
    );
    let first = operation.step(&mut hardware, &mut context);
    let second = operation.step(&mut hardware, &mut context);
    let result = operation.step(&mut hardware, &mut context);
    // Then: one child ran per step and the reducer changed the output type.
    assert_eq!(first, StepOutcome::Progressed);
    assert_eq!(second, StepOutcome::Progressed);
    assert_eq!(result, StepOutcome::Complete(Ok("11".to_owned())));
    assert_eq!(context, [8, 3]);
}

#[test]
fn invalid_plans_fail_before_any_selected_child_effect() {
    // Given: each malformed plan starts with a valid branch.
    for (plan, error) in [
        (vec![0, 2], BranchError::InvalidIndex { index: 2, ports: 2 }),
        (vec![0, 0], BranchError::DuplicateIndex { index: 0 }),
    ] {
        let mut hardware = FanIn::new(vec![Number(3), Number(8)], Sum(plan));
        let mut context = Vec::new();
        let mut operation = Operation::new(());
        // When: the operation starts gathering.
        let result = operation.step(&mut hardware, &mut context);
        // Then: validation rejects the entire plan without running its first child.
        assert_eq!(result, StepOutcome::Complete(Err(error)));
        assert!(context.is_empty());
    }
}

#[test]
fn empty_plan_reduces_without_child_interaction() {
    // Given: an application requests the empty aggregate.
    let mut hardware = FanIn::<Number, _>::new(Vec::new(), Sum(Vec::new()));
    let mut context = Vec::new();
    // When: the empty aggregate crosses planning and gathering before reduction.
    let mut operation = Operation::new(());
    assert_eq!(
        operation.step(&mut hardware, &mut context),
        StepOutcome::Progressed
    );
    assert_eq!(
        operation.step(&mut hardware, &mut context),
        StepOutcome::Progressed
    );
    let result = operation.step(&mut hardware, &mut context);
    // Then: the reducer receives an empty collection.
    assert_eq!(result, StepOutcome::Complete(Ok("0".to_owned())));
    assert!(context.is_empty());
}

enum Stop {
    Fail,
    Exhaust,
}

struct StopPolicy<'a> {
    outcome: Stop,
    reductions: &'a std::cell::Cell<usize>,
}

impl Combine<Vec<u8>, Number> for StopPolicy<'_> {
    type Output = ();
    type State = ();
    fn start() {}
    fn plan(
        &mut self,
        _: &Vec<u8>,
        _: &[Number],
        (): &mut (),
    ) -> StepOutcome<Vec<usize>, BranchError<&'static str>, String> {
        match self.outcome {
            Stop::Fail => StepOutcome::Complete(Err(BranchError::Application("policy fault"))),
            Stop::Exhaust => StepOutcome::Exhausted,
        }
    }
    fn combine(&mut self, (): (), _: Vec<u8>) -> Result<(), BranchError<&'static str>> {
        self.reductions.set(self.reductions.get() + 1);
        Ok(())
    }
}

#[test]
fn terminal_planning_stops_without_child_or_reducer_effects() {
    // Given: policies that fail or exhaust before accepting any child.
    for (outcome, expected) in [
        (
            Stop::Fail,
            StepOutcome::Complete(Err(BranchError::Application("policy fault"))),
        ),
        (Stop::Exhaust, StepOutcome::Exhausted),
    ] {
        let reductions = std::cell::Cell::new(0);
        let policy = StopPolicy {
            outcome,
            reductions: &reductions,
        };
        let mut hardware = FanIn::new(vec![Number(3)], policy);
        let mut context = Vec::new();
        let mut state: FanInState<(), bool, u8> =
            <FanIn<Number, StopPolicy<'_>> as Stage<Vec<u8>>>::start(());
        // When: the planning result is observed and its state is stepped again.
        let result = hardware.step(&mut context, &mut state);
        let terminal = hardware.step(&mut context, &mut state);
        // Then: the result is propagated once and all child work remains untouched.
        assert_eq!(result, expected);
        assert_eq!(terminal, StepOutcome::Exhausted);
        assert!(context.is_empty());
        assert_eq!(reductions.get(), 0);
    }
}
