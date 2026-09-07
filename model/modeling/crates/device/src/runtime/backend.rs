use std::convert::Infallible;
#[cfg(test)]
use std::sync::atomic::{AtomicUsize, Ordering};

use hardware_interface_pipeline::{AsyncInteractionBackend, StepOutcome};
use tokio::sync::{Mutex, watch};

use super::state::RunnerState;

#[derive(Debug)]
pub(crate) struct DeviceInteractionBackend {
    state: Mutex<RunnerState>,
    changed: watch::Sender<usize>,
    #[cfg(test)]
    generic_interactions: AtomicUsize,
    #[cfg(test)]
    generic_waits: AtomicUsize,
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum DeviceWait {
    Generation {
        generation: usize,
    },
    Wake {
        generation: usize,
        wake_generation: usize,
    },
}

impl DeviceWait {
    pub(crate) const fn generation(generation: usize) -> Self {
        Self::Generation { generation }
    }

    pub(crate) const fn wake(generation: usize, wake_generation: usize) -> Self {
        Self::Wake {
            generation,
            wake_generation,
        }
    }

    pub(crate) const fn generation_value(self) -> usize {
        match self {
            Self::Generation { generation } | Self::Wake { generation, .. } => generation,
        }
    }

    const fn unchanged(self, state: &RunnerState) -> bool {
        match self {
            Self::Generation { generation } => state.generation == generation,
            Self::Wake {
                wake_generation, ..
            } => state.wake_generation == wake_generation && !state.shutdown_requested,
        }
    }
}

impl DeviceInteractionBackend {
    pub(crate) fn new(state: RunnerState) -> Self {
        let (changed, _) = watch::channel(state.generation);
        Self {
            state: Mutex::new(state),
            changed,
            #[cfg(test)]
            generic_interactions: AtomicUsize::new(0),
            #[cfg(test)]
            generic_waits: AtomicUsize::new(0),
        }
    }

    pub(crate) async fn interact<T: Send>(
        &self,
        interaction: impl FnOnce(&mut RunnerState) -> T + Send,
    ) -> T {
        self.run_interaction(interaction).await
    }

    async fn run_interaction<T: Send>(
        &self,
        interaction: impl FnOnce(&mut RunnerState) -> T + Send,
    ) -> T {
        let mut state = self.state.lock().await;
        interaction(&mut state)
    }

    pub(crate) fn publish(&self, generation: usize) {
        self.changed.send_replace(generation);
    }

    #[cfg(test)]
    pub(crate) fn subscribe_for_test(&self) -> watch::Receiver<usize> {
        self.changed.subscribe()
    }

    #[cfg(test)]
    pub(crate) fn generic_boundaries_for_test(&self) -> (usize, usize) {
        (
            self.generic_interactions.load(Ordering::Relaxed),
            self.generic_waits.load(Ordering::Relaxed),
        )
    }

    #[cfg(test)]
    pub(crate) async fn hold_for_test(
        &self,
        ready: std::sync::mpsc::Sender<()>,
        release: std::sync::mpsc::Receiver<()>,
    ) {
        self.interact(move |_state| {
            let _ = ready.send(());
            let _ = release.recv();
        })
        .await;
    }

    pub(crate) async fn wait_for_change(&self, wait: DeviceWait) {
        match <Self as AsyncInteractionBackend<RunnerState, DeviceWait>>::wait(self, wait).await {
            Ok(()) => {}
            Err(error) => match error {},
        }
    }

    async fn wait_inner(&self, wait: DeviceWait) {
        let mut changes = self.changed.subscribe();
        loop {
            let unchanged = self.interact(|state| wait.unchanged(state)).await;
            if !unchanged {
                return;
            }
            if *changes.borrow_and_update() != wait.generation_value() {
                continue;
            }
            if changes.changed().await.is_err() {
                return;
            }
        }
    }
}

impl AsyncInteractionBackend<RunnerState, DeviceWait> for DeviceInteractionBackend {
    type Error = Infallible;

    async fn interact<O: Send, E: Send, F>(
        &self,
        interaction: F,
    ) -> Result<StepOutcome<O, E, DeviceWait>, Self::Error>
    where
        F: FnOnce(&mut RunnerState) -> StepOutcome<O, E, DeviceWait> + Send,
    {
        #[cfg(test)]
        self.generic_interactions.fetch_add(1, Ordering::Relaxed);
        let (outcome, publication) = self
            .run_interaction(|state| {
                let outcome = interaction(state);
                (outcome, state.worker_publication.take())
            })
            .await;
        if let Some(generation) = publication {
            self.publish(generation);
        }
        Ok(outcome)
    }

    async fn wait(&self, wait: DeviceWait) -> Result<(), Self::Error> {
        #[cfg(test)]
        self.generic_waits.fetch_add(1, Ordering::Relaxed);
        self.wait_inner(wait).await;
        Ok(())
    }
}

#[cfg(test)]
mod tests;
