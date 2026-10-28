//! Borrowed backend ownership and cooperative readiness.

use std::{convert::Infallible, sync::Mutex};

use hw_model_pipeline::{
    AsyncExecutor, AsyncInteractionBackend, Operation, Stage, StepOutcome, StepResult,
};

struct Backend(Mutex<u32>);

impl AsyncInteractionBackend<u32> for Backend {
    type Error = &'static str;

    async fn yield_now(&self) {
        tokio::task::yield_now().await;
    }

    async fn interact<O: Send, E: Send, F>(
        &self,
        interaction: F,
    ) -> Result<StepResult<O, E>, Self::Error>
    where
        F: FnOnce(&mut u32) -> StepResult<O, E> + Send,
    {
        let mut value = self.0.lock().map_err(|_| "poisoned interaction lock")?;
        Ok(interaction(&mut value))
    }
}

struct IncrementWhenReady;

impl Stage<u32> for IncrementWhenReady {
    type Input = ();
    type Output = u32;
    type Error = Infallible;

    fn step(
        &mut self,
        value: &mut u32,
        input: &mut Option<Self::Input>,
    ) -> StepResult<u32, Infallible> {
        if *value == 0 {
            return Ok(StepOutcome::InProgress);
        }
        let Some(()) = input.take() else {
            return Ok(StepOutcome::InProgress);
        };
        *value += 1;
        Ok(StepOutcome::Complete(Ok(*value)))
    }
}

#[tokio::test]
async fn executor_borrows_the_existing_backend_and_observes_software_readiness() {
    // Given: an externally owned backend whose value is supplied by software.
    let backend = Backend(Mutex::new(0));
    let mut stage = IncrementWhenReady;
    let mut operation = Operation::new(());

    // When: the executor drives the stage through a borrowed backend.
    let executor = AsyncExecutor::new(&backend);
    let (result, ()) = tokio::join!(biased; executor.run(&mut stage, &mut operation), async {
        *backend.0.lock().expect("software context access") = 7;
    });

    // Then: software and hardware mutate the original backend, which remains usable.
    assert_eq!(result, Ok(8));
    assert_eq!(*backend.0.lock().expect("backend is still owned here"), 8);
}
