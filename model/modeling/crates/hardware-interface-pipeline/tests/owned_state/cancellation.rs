use std::{
    convert::Infallible,
    future::{Future, pending},
    sync::{
        Arc, Mutex, PoisonError,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    task::{Context, Poll, Waker},
};

use hardware_interface_pipeline::{
    AsyncExecutor, AsyncInteractionBackend, Operation, Stage, StepOutcome,
};

struct Retained {
    text: String,
    drops: Arc<AtomicUsize>,
}

impl Drop for Retained {
    fn drop(&mut self) {
        self.drops.fetch_add(1, Ordering::SeqCst);
    }
}

struct Wait(Arc<AtomicUsize>);

impl Drop for Wait {
    fn drop(&mut self) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}

enum State {
    First(Retained),
    Last(Retained),
}
struct Work;

#[derive(Default)]
struct Device {
    effects: Vec<String>,
    ready: bool,
    wait_drops: Arc<AtomicUsize>,
}

impl Stage<Device> for Work {
    type Input = Retained;
    type Output = ();
    type Error = Infallible;
    type Wait = Wait;
    type State = State;
    fn start(input: Retained) -> State {
        State::First(input)
    }
    fn step(
        &mut self,
        context: &mut Device,
        state: State,
    ) -> StepOutcome<State, (), Infallible, Wait> {
        match state {
            State::First(state) => {
                context.effects.push(format!("first:{}", state.text));
                StepOutcome::Progressed(State::Last(state))
            }
            State::Last(state) if context.ready => {
                context.effects.push(format!("last:{}", state.text));
                StepOutcome::Complete(Ok(()))
            }
            State::Last(state) => {
                StepOutcome::Pending(State::Last(state), Wait(Arc::clone(&context.wait_drops)))
            }
        }
    }
}

struct Backend {
    before: AtomicBool,
    after: AtomicBool,
    device: Mutex<Device>,
}

impl AsyncInteractionBackend<Device, Wait> for Backend {
    type Error = Infallible;
    async fn interact<R: Send, F>(&self, interaction: F) -> Result<R, Infallible>
    where
        F: FnOnce(&mut Device) -> R + Send,
    {
        if !self.before.load(Ordering::SeqCst) {
            pending::<()>().await;
        }
        let result = interaction(&mut self.device.lock().unwrap_or_else(PoisonError::into_inner));
        if !self.after.load(Ordering::SeqCst) {
            pending::<()>().await;
        }
        Ok(result)
    }
    async fn wait(&self, wait: Wait) -> Result<(), Infallible> {
        pending::<()>().await;
        drop(wait);
        Ok(())
    }
}

fn executor(before: bool, after: bool) -> AsyncExecutor<Backend> {
    AsyncExecutor::new(Backend {
        before: AtomicBool::new(before),
        after: AtomicBool::new(after),
        device: Mutex::new(Device::default()),
    })
}

fn operation(drops: &Arc<AtomicUsize>) -> Operation<Work, Device> {
    Operation::new(Retained {
        text: "payload".into(),
        drops: Arc::clone(drops),
    })
}

fn cancel_pending(future: impl Future) {
    let mut future = Box::pin(future);
    assert!(matches!(
        future
            .as_mut()
            .poll(&mut Context::from_waker(Waker::noop())),
        Poll::Pending
    ));
}

#[tokio::test]
async fn cancel_before_acquisition_keeps_unstarted_owned_state() {
    // Given a backend blocked before taking context.
    let executor = executor(false, true);
    let drops = Arc::new(AtomicUsize::new(0));
    let mut operation = operation(&drops);
    let mut stage = Work;
    // When the driving future is dropped before acquisition.
    cancel_pending(executor.run(&mut stage, &mut operation));
    // Then request state survives with no effects and resumes normally.
    assert_eq!(drops.load(Ordering::SeqCst), 0);
    assert!(
        executor
            .backend()
            .device
            .lock()
            .expect("device")
            .effects
            .is_empty()
    );
    executor.backend().before.store(true, Ordering::SeqCst);
    executor.backend().device.lock().expect("device").ready = true;
    assert_eq!(executor.run(&mut stage, &mut operation).await, Ok(()));
    assert_eq!(
        executor.backend().device.lock().expect("device").effects,
        ["first:payload", "last:payload"]
    );
    assert_eq!(drops.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn cancel_after_interaction_retains_returned_continuation() {
    // Given a backend blocked after the first synchronous interaction.
    let executor = executor(true, false);
    let drops = Arc::new(AtomicUsize::new(0));
    let mut operation = operation(&drops);
    let mut stage = Work;
    // When the driving future is dropped after progress commits.
    cancel_pending(executor.run(&mut stage, &mut operation));
    // Then the continuation was restored before suspension and no effect repeats.
    assert_eq!(drops.load(Ordering::SeqCst), 0);
    assert_eq!(
        executor.backend().device.lock().expect("device").effects,
        ["first:payload"]
    );
    executor.backend().after.store(true, Ordering::SeqCst);
    executor.backend().device.lock().expect("device").ready = true;
    assert_eq!(executor.run(&mut stage, &mut operation).await, Ok(()));
    assert_eq!(
        executor.backend().device.lock().expect("device").effects,
        ["first:payload", "last:payload"]
    );
    assert_eq!(drops.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn cancel_wait_drops_token_but_retains_request() {
    // Given a request suspended after its first committed effect.
    let executor = executor(true, true);
    let drops = Arc::new(AtomicUsize::new(0));
    let mut operation = operation(&drops);
    let mut stage = Work;
    // When its backend wait is canceled.
    cancel_pending(executor.run(&mut stage, &mut operation));
    // Then only the wait token drops and resumed work completes exactly once.
    assert_eq!(drops.load(Ordering::SeqCst), 0);
    assert_eq!(
        executor
            .backend()
            .device
            .lock()
            .expect("device")
            .wait_drops
            .load(Ordering::SeqCst),
        1
    );
    executor.backend().device.lock().expect("device").ready = true;
    assert_eq!(executor.run(&mut stage, &mut operation).await, Ok(()));
    assert_eq!(
        executor.backend().device.lock().expect("device").effects,
        ["first:payload", "last:payload"]
    );
    assert_eq!(drops.load(Ordering::SeqCst), 1);
}
