//! Stage-owned input acceptance and cooperative completion boundaries.
use hw_model_pipeline::{Dispatch, FanOut, Route, Stage, StepOutcome, StepResult, Then};

#[derive(Default)]
struct Hold(Option<String>);

#[derive(Default)]
struct Effects {
    values: Vec<String>,
    reset: bool,
}

impl Stage<Effects> for Hold {
    type Input = String;
    type Output = String;
    type Error = ();

    fn step(
        &mut self,
        effects: &mut Effects,
        input: &mut Option<String>,
    ) -> StepResult<String, ()> {
        if effects.reset {
            if self.0.take().or_else(|| input.take()).is_none() {
                return Ok(StepOutcome::InProgress);
            }
            return Ok(StepOutcome::Complete(Err(())));
        }
        if let Some(value) = self.0.take() {
            effects.values.push(value.clone());
            return Ok(StepOutcome::Complete(Ok(value)));
        }
        self.0 = input.take();
        Ok(StepOutcome::InProgress)
    }
}

#[test]
fn busy_pipeline_preserves_next_offer_until_previous_completion() {
    // Given: a nested graph holding one accepted value.
    let mut graph = Then::new(Hold::default(), Then::new(Hold::default(), Hold::default()));
    let mut effects = Effects::default();
    let mut first = Some("first".to_owned());
    assert_eq!(
        graph.step(&mut effects, &mut first),
        Ok(StepOutcome::InProgress)
    );
    assert!(first.is_none());
    let mut next = Some("next".to_owned());
    // When: a replacement is offered throughout the first invocation.
    for _ in 0..4 {
        assert_eq!(
            graph.step(&mut effects, &mut next),
            Ok(StepOutcome::InProgress)
        );
        assert_eq!(next.as_deref(), Some("next"));
    }
    assert_eq!(
        graph.step(&mut effects, &mut next),
        Ok(StepOutcome::Complete(Ok("first".to_owned())))
    );
    // Then: completion did not accept the replacement; the next call does.
    assert_eq!(next.as_deref(), Some("next"));
    assert_eq!(effects.values, ["first", "first", "first"]);
    assert_eq!(
        graph.step(&mut effects, &mut next),
        Ok(StepOutcome::InProgress)
    );
    assert!(next.is_none());
}

#[test]
fn stage_observes_reset_and_releases_storage_without_undoing_effects() {
    // Given: upstream has committed and a nested downstream holds its value.
    let mut graph = Then::new(Hold::default(), Then::new(Hold::default(), Hold::default()));
    let mut effects = Effects::default();
    let mut input = Some("old".to_owned());
    for _ in 0..3 {
        assert_eq!(
            graph.step(&mut effects, &mut input),
            Ok(StepOutcome::InProgress)
        );
    }
    // When: the active child observes a domain reset and finishes its invocation.
    effects.reset = true;
    let mut next = Some("next".to_owned());
    assert_eq!(
        graph.step(&mut effects, &mut next),
        Ok(StepOutcome::Complete(Err(())))
    );
    assert_eq!(next.as_deref(), Some("next"));
    assert_eq!(
        graph.step(&mut effects, &mut None),
        Ok(StepOutcome::InProgress)
    );
    effects.reset = false;
    input = Some("new".to_owned());
    for _ in 0..5 {
        assert_eq!(
            graph.step(&mut effects, &mut input),
            Ok(StepOutcome::InProgress)
        );
    }
    let result = graph.step(&mut effects, &mut input);
    // Then: only the committed old effect survives; fresh work traverses every child.
    assert_eq!(result, Ok(StepOutcome::Complete(Ok("new".to_owned()))));
    assert_eq!(effects.values, ["old", "new", "new", "new"]);
}

struct First;
impl Route<String, ()> for First {
    fn target(&mut self, _: &String) -> Result<usize, ()> {
        Ok(0)
    }
}

#[test]
fn fan_keeps_new_offer_while_source_and_destination_are_active() {
    // Given: a fan with a delayed source and destination.
    let mut fan = FanOut::new(Hold::default(), Dispatch::new(First), vec![Hold::default()]);
    let mut effects = Effects::default();
    let mut input = Some("first".to_owned());
    assert_eq!(
        fan.step(&mut effects, &mut input),
        Ok(StepOutcome::InProgress)
    );
    input = Some("next".to_owned());
    // When: a new offer remains present through the active fan's terminal step.
    for _ in 0..3 {
        assert_eq!(
            fan.step(&mut effects, &mut input),
            Ok(StepOutcome::InProgress)
        );
        assert_eq!(input.as_deref(), Some("next"));
    }
    let result = fan.step(&mut effects, &mut input);
    // Then: both children used the first value once and the new offer remains owned by the caller.
    assert_eq!(
        result,
        Ok(StepOutcome::Complete(Ok(vec![(0, "first".to_owned())])))
    );
    assert_eq!(input.as_deref(), Some("next"));
    assert_eq!(effects.values, ["first", "first"]);
}

struct Optional;
impl Stage<usize> for Optional {
    type Input = Option<String>;
    type Output = Option<String>;
    type Error = ();

    fn step(
        &mut self,
        effects: &mut usize,
        input: &mut Option<Self::Input>,
    ) -> StepResult<Self::Output, ()> {
        let Some(value) = input.take() else {
            return Ok(StepOutcome::InProgress);
        };
        *effects += 1;
        Ok(StepOutcome::Complete(Ok(value)))
    }
}

#[test]
fn no_offer_is_idle_but_optional_none_traverses_the_graph() {
    // Given: an idle graph whose domain accepts optional values.
    let mut graph = Then::new(Optional, Optional);
    let mut effects = 0;
    assert_eq!(
        graph.step(&mut effects, &mut None),
        Ok(StepOutcome::InProgress)
    );
    assert_eq!(effects, 0);
    // When: an actual domain None is offered and passed downstream.
    let mut input = Some(None);
    assert_eq!(
        graph.step(&mut effects, &mut input),
        Ok(StepOutcome::InProgress)
    );
    let result = graph.step(&mut effects, &mut input);
    // Then: both stages accept the domain value and idle stepping has no effect.
    assert_eq!(result, Ok(StepOutcome::Complete(Ok(None))));
    assert_eq!(effects, 2);
    assert_eq!(
        graph.step(&mut effects, &mut None),
        Ok(StepOutcome::InProgress)
    );
    assert_eq!(effects, 2);
}

#[derive(Default)]
struct Ownership {
    resident: usize,
    committed: usize,
    reset: bool,
}

#[derive(Default)]
struct Resident(bool);
impl Stage<Ownership> for Resident {
    type Input = ();
    type Output = ();
    type Error = ();

    fn step(&mut self, context: &mut Ownership, input: &mut Option<()>) -> StepResult<(), ()> {
        if self.0 {
            self.0 = false;
            context.resident -= 1;
            if context.reset {
                return Ok(StepOutcome::Complete(Err(())));
            }
            context.committed += 1;
            return Ok(StepOutcome::Complete(Ok(())));
        }
        if input.take().is_some() {
            self.0 = true;
            context.resident += 1;
        }
        Ok(StepOutcome::InProgress)
    }
}

#[test]
fn terminal_step_releases_context_residency_after_clearing_child_storage() {
    // Given: one committed upstream effect and one resident downstream invocation.
    let mut graph = Then::new(Resident::default(), Resident::default());
    let mut context = Ownership::default();
    let mut input = Some(());
    for _ in 0..3 {
        assert_eq!(
            graph.step(&mut context, &mut input),
            Ok(StepOutcome::InProgress)
        );
    }
    assert_eq!(context.resident, 1);
    assert_eq!(context.committed, 1);
    // When: the resident child observes reset and releases its own ownership.
    context.reset = true;
    assert_eq!(context.resident, 1);
    assert_eq!(
        graph.step(&mut context, &mut input),
        Ok(StepOutcome::Complete(Err(())))
    );
    // Then: no resident remains and the committed effect is preserved.
    assert_eq!(context.resident, 0);
    assert_eq!(context.committed, 1);
}
