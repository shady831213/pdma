use std::convert::Infallible;

use hardware_interface_pipeline::{
    AsyncExecutor, AsyncInteractionBackend, AsyncRunError, StepOutcome,
};

use super::{Drive, Driven, finish_observation};
use crate::device::command_pipeline::tests::support::Prepared;

pub(super) async fn observe(mut prepared: Prepared) -> Driven {
    let terminal = {
        let executor = AsyncExecutor::new(DirectBackend);
        match executor
            .run(&mut prepared.fixture.core, &mut prepared.fixture.operation)
            .await
        {
            Ok(status) => Ok(status),
            Err(AsyncRunError::Stage(error)) => Err(error),
            Err(AsyncRunError::Exhausted) => panic!("fresh async pipeline exhausted"),
            Err(AsyncRunError::BackendInteraction(error) | AsyncRunError::BackendWait(error)) => {
                match error {}
            }
        }
    };
    finish_observation(prepared, terminal, 0, Drive::Asynchronous)
}

struct DirectBackend;

impl AsyncInteractionBackend<(), crate::AsyncStallReason> for DirectBackend {
    type Error = Infallible;

    async fn interact<O: Send, E: Send, F>(
        &self,
        interaction: F,
    ) -> Result<StepOutcome<O, E, crate::AsyncStallReason>, Self::Error>
    where
        F: FnOnce(&mut ()) -> StepOutcome<O, E, crate::AsyncStallReason> + Send,
    {
        Ok(interaction(&mut ()))
    }

    async fn wait(&self, _wait: crate::AsyncStallReason) -> Result<(), Self::Error> {
        Ok(())
    }
}
