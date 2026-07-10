//! Modeled software execution contexts sharing one CPU.

use std::{future::Future, sync::Arc};
use tokio::sync::Mutex;

/// Single modeled CPU token shared by SW main and IRQ handler contexts.
#[derive(Debug, Clone, Default)]
pub struct SoftwareCpu {
    gate: Arc<Mutex<()>>,
}

impl SoftwareCpu {
    /// Creates one idle modeled CPU.
    pub fn new() -> Self {
        Self::default()
    }

    /// Runs one SW-main turn without allowing an IRQ handler to overlap it.
    pub async fn run_main<F, T>(&self, turn: F) -> T
    where
        F: Future<Output = T>,
    {
        self.run_exclusive(turn).await
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
