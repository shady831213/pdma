use std::convert::Infallible;

use hardware_interface_pipeline::{
    AsyncExecutor, AsyncInteractionBackend, AsyncRunError, StepOutcome,
};

use super::{Drive, Driven, finish_observation};
use crate::device::command_pipeline::tests::support::Prepared;

pub(super) async fn observe(mut prepared: Prepared) -> Driven {
    let terminal = {
        let backend = DirectBackend {
            core: tokio::sync::Mutex::new(&mut prepared.fixture.core),
        };
        let executor = AsyncExecutor::new(backend);
        match executor.run(&mut prepared.fixture.pipeline).await {
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

struct DirectBackend<'a> {
    core: tokio::sync::Mutex<&'a mut crate::device::DeviceCore>,
}

impl AsyncInteractionBackend<crate::device::DeviceCore, crate::AsyncStallReason>
    for DirectBackend<'_>
{
    type Error = Infallible;

    async fn interact<O: Send, E: Send, F>(
        &self,
        interaction: F,
    ) -> Result<StepOutcome<O, E, crate::AsyncStallReason>, Self::Error>
    where
        F: FnOnce(&mut crate::device::DeviceCore) -> StepOutcome<O, E, crate::AsyncStallReason>
            + Send,
    {
        let mut core = self.core.lock().await;
        Ok(interaction(&mut core))
    }

    async fn wait(&self, _wait: crate::AsyncStallReason) -> Result<(), Self::Error> {
        Ok(())
    }
}
