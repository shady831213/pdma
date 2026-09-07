//! Borrowed backend ownership and wait/resume behavior.

use std::{convert::Infallible, sync::Mutex};

use hardware_interface_pipeline::{
    AsyncExecutor, AsyncInteractionBackend, Operation, Stage, StepOutcome,
};

struct Backend(Mutex<u32>);

impl AsyncInteractionBackend<u32, ()> for Backend {
    type Error = &'static str;

    async fn interact<O: Send, E: Send, F>(
        &self,
        interaction: F,
    ) -> Result<StepOutcome<O, E, ()>, Self::Error>
    where
        F: FnOnce(&mut u32) -> StepOutcome<O, E, ()> + Send,
    {
        let mut value = self.0.lock().map_err(|_| "poisoned interaction lock")?;
        Ok(interaction(&mut value))
    }

    async fn wait(&self, (): ()) -> Result<(), Self::Error> {
        *self.0.lock().map_err(|_| "poisoned wait lock")? = 7;
        Ok(())
    }
}

struct IncrementWhenReady;

impl Stage<u32> for IncrementWhenReady {
    type Input = ();
    type Output = u32;
    type Error = Infallible;
    type Wait = ();
    type State = ();

    fn start((): Self::Input) -> Self::State {}

    fn step(&mut self, value: &mut u32, (): &mut Self::State) -> StepOutcome<u32, Infallible, ()> {
        if *value == 0 {
            return StepOutcome::Pending(());
        }
        *value += 1;
        StepOutcome::Complete(Ok(*value))
    }
}

#[tokio::test]
async fn executor_borrows_the_existing_backend_and_resumes_on_its_wait() {
    // Given: an externally owned backend whose value becomes available on wait.
    let backend = Backend(Mutex::new(0));
    let mut stage = IncrementWhenReady;
    let mut operation = Operation::new(());

    // When: the executor drives the stage through a borrowed backend.
    let result = AsyncExecutor::new(&backend)
        .run(&mut stage, &mut operation)
        .await;

    // Then: waiting and execution mutate the original backend, which remains usable.
    assert_eq!(result, Ok(8));
    assert_eq!(*backend.0.lock().expect("backend is still owned here"), 8);
}
