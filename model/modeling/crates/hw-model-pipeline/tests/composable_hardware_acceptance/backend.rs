use std::sync::{
    Mutex,
    atomic::{AtomicUsize, Ordering},
};

use hw_model_pipeline::{AsyncInteractionBackend, StepResult};

use super::stages::Environment;

#[derive(Debug, Default)]
pub struct Backend {
    environment: Mutex<Environment>,
    blocked_interactions: AtomicUsize,
}

impl Backend {
    pub fn open(&self) -> Result<(), &'static str> {
        self.environment
            .lock()
            .map_err(|_| "poisoned environment")?
            .writable = true;
        Ok(())
    }

    pub fn published(&self) -> Result<Vec<u64>, &'static str> {
        Ok(self
            .environment
            .lock()
            .map_err(|_| "poisoned environment")?
            .responses
            .clone())
    }

    pub fn blocked_interactions(&self) -> usize {
        self.blocked_interactions.load(Ordering::Relaxed)
    }
}

impl AsyncInteractionBackend<Environment> for Backend {
    type Error = &'static str;

    async fn yield_now(&self) {
        tokio::task::yield_now().await;
    }

    async fn interact<O, E, F>(&self, interaction: F) -> Result<StepResult<O, E>, Self::Error>
    where
        O: Send,
        E: Send,
        F: FnOnce(&mut Environment) -> StepResult<O, E> + Send,
    {
        let mut environment = self
            .environment
            .lock()
            .map_err(|_| "poisoned environment")?;
        if !environment.writable {
            self.blocked_interactions.fetch_add(1, Ordering::Relaxed);
        }
        let outcome = interaction(&mut environment);
        drop(environment);
        Ok(outcome)
    }
}
