//! Retained owned values and child failures during gathering.
use hardware_interface_pipeline::{
    BranchError, FanIn, FanPlan, FanPolicy, RunError, Stage, StepOutcome, StepResult,
};
#[derive(Default)]
struct Context {
    consumed: Vec<u8>,
    released: bool,
    outputs: usize,
}
struct Owned(String);
struct Source(u8);
enum SourceState {
    New,
    Waiting(Owned),
    Done,
}
impl Stage<Context> for Source {
    type Input = ();
    type Output = Owned;
    type Error = u8;
    type State = SourceState;
    fn start((): ()) -> SourceState {
        SourceState::New
    }
    fn step(&mut self, context: &mut Context, state: &mut SourceState) -> StepResult<Owned, u8> {
        match std::mem::replace(state, SourceState::Done) {
            SourceState::New => {
                context.consumed.push(self.0);
                match self.0 {
                    0 => Ok(StepOutcome::Complete(Ok(Owned("first".into())))),
                    1 => {
                        *state = SourceState::Waiting(Owned("second".into()));
                        Ok(StepOutcome::InProgress)
                    }
                    2 => Ok(StepOutcome::Complete(Err(42))),
                    _ => Err(RunError::Exhausted),
                }
            }
            SourceState::Waiting(value) => {
                if context.released {
                    Ok(StepOutcome::Complete(Ok(value)))
                } else {
                    *state = SourceState::Waiting(value);
                    Ok(StepOutcome::InProgress)
                }
            }
            SourceState::Done => Err(RunError::Exhausted),
        }
    }
}
struct Gather {
    fail: bool,
}
impl FanPolicy<Owned, String, u8> for Gather {
    fn plan(&mut self, values: &[Option<Owned>], _: &[bool]) -> Result<FanPlan, u8> {
        if values.iter().any(Option::is_none) {
            return Ok(FanPlan::default());
        }
        Ok(FanPlan {
            inputs: (0..values.len()).collect(),
            outputs: vec![0],
        })
    }
    fn route(&mut self, values: Vec<Owned>) -> Result<Vec<String>, u8> {
        if self.fail {
            Err(19)
        } else {
            Ok(vec![values.into_iter().map(|v| v.0).collect()])
        }
    }
}
struct Sink;
impl Stage<Context> for Sink {
    type Input = String;
    type Output = String;
    type Error = u8;
    type State = Option<String>;
    fn start(input: String) -> Self::State {
        Some(input)
    }
    fn step(&mut self, context: &mut Context, state: &mut Self::State) -> StepResult<String, u8> {
        let value = state.take().ok_or(RunError::Exhausted)?;
        context.outputs += 1;
        Ok(StepOutcome::Complete(Ok(value)))
    }
}
type Pipeline = FanIn<Source, Gather, Sink>;
#[test]
fn partial_gather_keeps_owned_results_across_incomplete_inputs() {
    // Given: a non-Clone completed value and a delayed peer.
    let mut hardware = FanIn::new(vec![Source(0), Source(1)], Gather { fail: false }, Sink);
    let mut state = Pipeline::start(vec![(); 2]);
    let mut context = Context::default();
    for _ in 0..6 {
        assert!(matches!(
            hardware.step(&mut context, &mut state),
            Ok(StepOutcome::InProgress)
        ));
    }
    // When: the delayed source becomes runnable.
    context.released = true;
    assert!(matches!(
        hardware.step(&mut context, &mut state),
        Ok(StepOutcome::InProgress)
    ));
    assert!(matches!(
        hardware.step(&mut context, &mut state),
        Ok(StepOutcome::InProgress)
    ));
    let result = hardware.step(&mut context, &mut state);
    // Then: both original owned values reach reduction without source replay.
    assert_eq!(
        result,
        Ok(StepOutcome::Complete(Ok(vec![(0, "firstsecond".into())])))
    );
    assert_eq!(context.consumed, [0, 1]);
    assert_eq!(context.outputs, 1);
}
#[test]
fn reducer_failure_terminalizes_without_output_effects() {
    // Given: reduction rejects a completed source value.
    let mut hardware = FanIn::new(vec![Source(0)], Gather { fail: true }, Sink);
    let mut state = Pipeline::start(vec![()]);
    let mut context = Context::default();
    assert!(matches!(
        hardware.step(&mut context, &mut state),
        Ok(StepOutcome::InProgress)
    ));
    // When: the validated selection is reduced.
    let result = hardware.step(&mut context, &mut state);
    // Then: the error is returned once and output hardware remains untouched.
    assert_eq!(
        result,
        Ok(StepOutcome::Complete(Err(BranchError::Application(19))))
    );
    assert_eq!(context.outputs, 0);
    assert!(state.is_none());
    assert_eq!(
        hardware.step(&mut context, &mut state),
        Err(RunError::Exhausted)
    );
}
#[test]
fn child_failure_and_exhaustion_stop_later_input_effects() {
    // Given: a completed branch precedes a failing source and a later source.
    for (id, expected) in [
        (
            2,
            Ok(StepOutcome::Complete(Err(BranchError::Application(42)))),
        ),
        (3, Err(RunError::Exhausted)),
    ] {
        let mut hardware = FanIn::new(
            vec![Source(0), Source(id), Source(1)],
            Gather { fail: false },
            Sink,
        );
        let mut state = Pipeline::start(vec![(); 3]);
        let mut context = Context::default();
        // When: the input sweep encounters the terminal child.
        let result = hardware.step(&mut context, &mut state);
        // Then: committed effects remain, later children are untouched, and invocation state is empty.
        assert_eq!(result, expected);
        assert_eq!(context.consumed, [0, id]);
        assert_eq!(context.outputs, 0);
        assert!(state.is_none());
    }
}
