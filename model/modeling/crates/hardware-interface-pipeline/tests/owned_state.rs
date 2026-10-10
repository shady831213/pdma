//! Public owned-state lifecycle regression tests.

use std::{
    convert::Infallible,
    sync::atomic::{AtomicUsize, Ordering},
};

use hardware_interface_pipeline::{
    AsyncExecutor, AsyncInteractionBackend, AsyncRunError, Operation, Stage, StepOutcome,
};

mod owned_state {
    mod binding;
    mod cancellation;
    mod resume;
}

struct Finish;

impl Stage<()> for Finish {
    type Input = ();
    type Output = ();
    type Error = Infallible;
    type Wait = ();
    type State = ();
    fn start((): ()) {}
    fn step(&mut self, (): &mut (), (): ()) -> StepOutcome<(), (), Infallible, ()> {
        StepOutcome::Complete(Ok(()))
    }
}

#[derive(Default)]
struct CountingBackend(AtomicUsize);

impl AsyncInteractionBackend<(), ()> for CountingBackend {
    type Error = Infallible;
    async fn interact<R: Send, F>(&self, interaction: F) -> Result<R, Infallible>
    where
        F: FnOnce(&mut ()) -> R + Send,
    {
        self.0.fetch_add(1, Ordering::SeqCst);
        Ok(interaction(&mut ()))
    }
    async fn wait(&self, (): ()) -> Result<(), Infallible> {
        Ok(())
    }
}

#[tokio::test]
async fn completed_operation_never_acquires_backend_again() {
    // Given a completed operation and a backend recording every acquisition.
    let executor = AsyncExecutor::new(CountingBackend::default());
    let mut stage = Finish;
    let mut operation = Operation::new(());
    assert_eq!(executor.run(&mut stage, &mut operation).await, Ok(()));
    let before = executor.backend().0.load(Ordering::SeqCst);
    // When the caller accidentally drives the completed operation again.
    assert_eq!(
        executor.run(&mut stage, &mut operation).await,
        Err(AsyncRunError::AlreadyComplete)
    );
    // Then no backend acquisition occurs.
    assert_eq!(executor.backend().0.load(Ordering::SeqCst), before);
}
