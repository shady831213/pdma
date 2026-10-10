//! Retained application state and partial fan-in ownership.

use hardware_interface_pipeline::{
    BranchError, Combine, FanIn, Operation, RunOutcome, Stage, StepOutcome,
};

#[derive(Default)]
struct Context {
    consumed: Vec<u8>,
    released: bool,
}
struct Owned(String);
struct Wait;
struct Source(u8);

impl Stage<Context> for Source {
    type Input = ();
    type Output = Owned;
    type Error = u8;
    type Wait = Wait;
    type State = Option<Owned>;
    fn start((): ()) -> Self::State {
        None
    }
    fn step(
        &mut self,
        context: &mut Context,
        mut state: Self::State,
    ) -> StepOutcome<Self::State, Owned, u8, Wait> {
        match state.take() {
            Some(value) if context.released => StepOutcome::Complete(Ok(value)),
            Some(value) => {
                state = Some(value);
                StepOutcome::Pending(state, Wait)
            }
            None => {
                context.consumed.push(self.0);
                match self.0 {
                    0 => StepOutcome::Complete(Ok(Owned("first".to_owned()))),
                    1 => {
                        state = Some(Owned("second".to_owned()));
                        StepOutcome::Progressed(state)
                    }
                    2 => StepOutcome::Complete(Err(42)),
                    _ => StepOutcome::Complete(Err(43)),
                }
            }
        }
    }
}

struct Gather {
    fail: bool,
}

impl Combine<Context, Source> for Gather {
    type Output = String;
    type State = usize;
    fn start() -> usize {
        0
    }
    fn plan(
        &mut self,
        _: &Context,
        sources: &[Source],
        mut state: usize,
    ) -> StepOutcome<Self::State, (Self::State, Vec<usize>), BranchError<u8>, Wait> {
        state += 1;
        if state == 1 {
            StepOutcome::Pending(state, Wait)
        } else {
            StepOutcome::Complete(Ok((state, (0..sources.len()).collect())))
        }
    }
    fn combine(&mut self, state: usize, outputs: Vec<Owned>) -> Result<String, BranchError<u8>> {
        assert_eq!(state, 2);
        if self.fail {
            Err(BranchError::Application(19))
        } else {
            Ok(outputs.into_iter().map(|value| value.0).collect())
        }
    }
}

#[test]
fn partial_gather_keeps_owned_results_and_planning_state_across_pending() {
    // Given: non-Clone output and wait types, with a delayed second source.
    let mut hardware = FanIn::new(vec![Source(0), Source(1)], Gather { fail: false });
    let mut context = Context::default();
    let mut operation = Operation::new(());
    // When: planning waits, one result is gathered, and the second source suspends.
    let planning = operation.step(&mut hardware, &mut context);
    assert!(matches!(
        operation.step(&mut hardware, &mut context),
        Ok(RunOutcome::Progressed)
    ));
    let first = operation.step(&mut hardware, &mut context);
    let accepted = operation.step(&mut hardware, &mut context);
    let pending = operation.step(&mut hardware, &mut context);
    context.released = true;
    assert!(matches!(
        operation.step(&mut hardware, &mut context),
        Ok(RunOutcome::Progressed)
    ));
    let result = operation.step(&mut hardware, &mut context);
    let terminal = operation.step(&mut hardware, &mut context);
    // Then: resume retains the first output and does not restart either source.
    assert!(matches!(planning, Ok(RunOutcome::Pending(Wait))));
    assert!(matches!(first, Ok(RunOutcome::Progressed)));
    assert!(matches!(accepted, Ok(RunOutcome::Progressed)));
    assert!(matches!(pending, Ok(RunOutcome::Pending(Wait))));
    assert!(matches!(result, Ok(RunOutcome::Complete(Ok(value))) if value == "firstsecond"));
    assert!(matches!(
        terminal,
        Err(hardware_interface_pipeline::AlreadyComplete)
    ));
    assert_eq!(context.consumed, [0, 1]);
}

#[test]
fn reducer_failure_terminalizes_the_operation() {
    // Given: an application reducer that rejects an otherwise successful gather.
    let mut hardware = FanIn::new(vec![Source(0)], Gather { fail: true });
    let mut context = Context::default();
    let mut state = Operation::new(());
    // When: the policy resumes and reduction fails.
    let planning = state.step(&mut hardware, &mut context);
    assert!(matches!(
        state.step(&mut hardware, &mut context),
        Ok(RunOutcome::Progressed)
    ));
    assert!(matches!(
        state.step(&mut hardware, &mut context),
        Ok(RunOutcome::Progressed)
    ));
    let result = state.step(&mut hardware, &mut context);
    let terminal = state.step(&mut hardware, &mut context);
    // Then: the typed reducer error is returned exactly once.
    assert!(matches!(planning, Ok(RunOutcome::Pending(Wait))));
    assert!(matches!(
        result,
        Ok(RunOutcome::Complete(Err(BranchError::Application(19))))
    ));
    assert!(matches!(
        terminal,
        Err(hardware_interface_pipeline::AlreadyComplete)
    ));
    assert_eq!(context.consumed, [0]);
}

#[test]
fn child_failure_and_missing_work_stop_later_selected_branches() {
    // Given: a completed branch, a terminal branch, and a later branch.
    for terminal_id in [2, 3] {
        let mut hardware = FanIn::new(
            vec![Source(0), Source(terminal_id), Source(1)],
            Gather { fail: false },
        );
        let mut context = Context::default();
        let mut state = Operation::new(());
        // When: the selected terminal child is reached after planning and the first child.
        let planning = state.step(&mut hardware, &mut context);
        assert!(matches!(
            state.step(&mut hardware, &mut context),
            Ok(RunOutcome::Progressed)
        ));
        let first = state.step(&mut hardware, &mut context);
        let result = state.step(&mut hardware, &mut context);
        let terminal = state.step(&mut hardware, &mut context);
        // Then: later children remain unstarted and repeated stepping is inert.
        assert!(matches!(planning, Ok(RunOutcome::Pending(Wait))));
        assert!(matches!(first, Ok(RunOutcome::Progressed)));
        match terminal_id {
            2 => assert!(matches!(
                result,
                Ok(RunOutcome::Complete(Err(BranchError::Application(42))))
            )),
            _ => assert!(matches!(
                result,
                Ok(RunOutcome::Complete(Err(BranchError::Application(43))))
            )),
        }
        assert!(matches!(
            terminal,
            Err(hardware_interface_pipeline::AlreadyComplete)
        ));
        assert_eq!(context.consumed, [0, terminal_id]);
    }
}
