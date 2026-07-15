//! Modeled software execution contexts sharing one CPU.

use std::{
    future::{Future, poll_fn},
    pin::Pin,
    sync::Arc,
    task::ready,
};
use tokio::sync::{Mutex, OwnedMutexGuard};

#[cfg(test)]
mod tests;

/// Single modeled CPU token shared by SW main and IRQ handler contexts.
#[derive(Debug, Clone, Default)]
pub struct SoftwareCpu {
    gate: Arc<Mutex<()>>,
}

type CpuAcquisition = Pin<Box<dyn Future<Output = OwnedMutexGuard<()>> + Send>>;

impl SoftwareCpu {
    /// Creates one idle modeled CPU.
    pub fn new() -> Self {
        Self::default()
    }

    /// Runs one complete SW-main program as atomic poll-to-pending segments.
    pub async fn run_main<F, T>(&self, program: F) -> T
    where
        F: Future<Output = T>,
    {
        let gate = Arc::clone(&self.gate);
        let mut program = Box::pin(program);
        let mut acquisition: Option<CpuAcquisition> = None;
        poll_fn(move |context| {
            let acquire =
                acquisition.get_or_insert_with(|| Box::pin(Arc::clone(&gate).lock_owned()));
            let cpu = ready!(acquire.as_mut().poll(context));
            acquisition = None;
            let state = program.as_mut().poll(context);
            drop(cpu);
            state
        })
        .await
    }

    pub(crate) async fn run_irq<F, T>(&self, handler: F) -> T
    where
        F: Future<Output = T>,
    {
        self.run_exclusive(handler).await
    }

    async fn run_exclusive<F, T>(&self, work: F) -> T
    where
        F: Future<Output = T>,
    {
        let _cpu = Arc::clone(&self.gate).lock_owned().await;
        work.await
    }
}
