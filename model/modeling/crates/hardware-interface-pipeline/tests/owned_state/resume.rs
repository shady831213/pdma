use std::{
    convert::Infallible,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};

use hardware_interface_pipeline::{AlreadyComplete, Operation, RunOutcome, Stage, StepOutcome};

#[derive(Debug)]
struct Request {
    payload: String,
    starts: Arc<AtomicUsize>,
    drops: Arc<AtomicUsize>,
}

impl Drop for Request {
    fn drop(&mut self) {
        self.drops.fetch_add(1, Ordering::SeqCst);
    }
}

#[derive(Debug)]
enum State {
    First(Request),
    Resume(Request),
}

#[derive(Debug)]
enum Turn {
    Suspended(State),
    Finished(String),
}

struct Transfer;

impl Stage<Vec<String>> for Transfer {
    type Input = Request;
    type Output = Turn;
    type Error = Infallible;
    type Wait = Infallible;
    type State = State;

    fn start(request: Request) -> State {
        request.starts.fetch_add(1, Ordering::SeqCst);
        State::First(request)
    }

    fn step(
        &mut self,
        effects: &mut Vec<String>,
        state: State,
    ) -> StepOutcome<State, Turn, Infallible, Infallible> {
        match state {
            State::First(mut request) => {
                effects.push(format!("read:{}", request.payload));
                request.payload.push_str("/retained");
                StepOutcome::Complete(Ok(Turn::Suspended(State::Resume(request))))
            }
            State::Resume(mut request) => {
                effects.push(format!("write:{}", request.payload));
                StepOutcome::Complete(Ok(Turn::Finished(std::mem::take(&mut request.payload))))
            }
        }
    }
}

#[test]
fn returned_nonclone_continuation_resumes_without_restart_or_replay()
-> Result<(), Box<dyn std::error::Error>> {
    // Given a completed execution turn that returns its non-Clone continuation.
    let starts = Arc::new(AtomicUsize::new(0));
    let drops = Arc::new(AtomicUsize::new(0));
    let mut stage = Transfer;
    let mut effects = Vec::new();
    let mut first = Operation::new(Request {
        payload: "input".into(),
        starts: Arc::clone(&starts),
        drops: Arc::clone(&drops),
    });
    let RunOutcome::Complete(Ok(Turn::Suspended(continuation))) =
        first.step(&mut stage, &mut effects)?
    else {
        return Err("first turn did not return its continuation".into());
    };
    assert!(first.is_complete());
    assert_eq!(effects, ["read:input"]);
    assert_eq!(drops.load(Ordering::SeqCst), 0);
    // When a fresh operation consumes the returned continuation.
    let mut resumed = Operation::from_state(continuation);
    assert!(!resumed.is_complete());
    assert_eq!(starts.load(Ordering::SeqCst), 1);
    let RunOutcome::Complete(Ok(Turn::Finished(output))) =
        resumed.step(&mut stage, &mut effects)?
    else {
        return Err("resumed turn did not finish".into());
    };
    // Then the mutated payload and exact effects survive, with one terminal drop.
    assert_eq!(output, "input/retained");
    assert_eq!(effects, ["read:input", "write:input/retained"]);
    assert_eq!(starts.load(Ordering::SeqCst), 1);
    assert_eq!(drops.load(Ordering::SeqCst), 1);
    assert!(resumed.is_complete());
    assert!(matches!(
        resumed.step(&mut stage, &mut effects),
        Err(AlreadyComplete)
    ));
    assert_eq!(effects.len(), 2);
    drop(first);
    drop(resumed);
    assert_eq!(drops.load(Ordering::SeqCst), 1);
    Ok(())
}
