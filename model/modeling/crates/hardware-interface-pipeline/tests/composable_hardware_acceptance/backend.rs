use std::{
    future::poll_fn,
    sync::{
        Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    task::{Poll, Waker},
};

use hardware_interface_pipeline::{AsyncInteractionBackend, StepOutcome};

use super::stages::Environment;

#[derive(Debug, Default)]
pub struct Backend {
    environment: Mutex<Environment>,
    waiter: Mutex<Option<Waker>>,
    waits: AtomicUsize,
}

impl Backend {
    pub fn open(&self) -> Result<(), &'static str> {
        self.environment
            .lock()
            .map_err(|_| "poisoned environment")?
            .writable = true;
        let waiter = self.waiter.lock().map_err(|_| "poisoned waiter")?.take();
        if let Some(waiter) = waiter {
            waiter.wake();
        }
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

    pub fn waits(&self) -> usize {
        self.waits.load(Ordering::Relaxed)
    }
}

impl AsyncInteractionBackend<Environment, ()> for Backend {
    type Error = &'static str;

    async fn interact<O, E, F>(&self, interaction: F) -> Result<StepOutcome<O, E, ()>, Self::Error>
    where
        O: Send,
        E: Send,
        F: FnOnce(&mut Environment) -> StepOutcome<O, E, ()> + Send,
    {
        let mut environment = self
            .environment
            .lock()
            .map_err(|_| "poisoned environment")?;
        Ok(interaction(&mut environment))
    }

    async fn wait(&self, (): ()) -> Result<(), Self::Error> {
        poll_fn(|context| {
            let mut waiter = self.waiter.lock().map_err(|_| "poisoned waiter")?;
            if self
                .environment
                .lock()
                .map_err(|_| "poisoned environment")?
                .writable
            {
                Poll::Ready(Ok(()))
            } else {
                self.waits.fetch_add(1, Ordering::Relaxed);
                *waiter = Some(context.waker().clone());
                Poll::Pending
            }
        })
        .await
    }
}
