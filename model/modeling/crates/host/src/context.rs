//! Modeled software execution contexts sharing one CPU.

use std::{
    cell::RefCell,
    future::{Future, poll_fn},
    pin::Pin,
    sync::Arc,
    task::ready,
};
use tokio::sync::{Mutex, OwnedMutexGuard};

mod cpu_local;
#[cfg(test)]
mod tests;

pub use cpu_local::CpuLocal;

#[derive(Debug)]
struct SoftwareCpuToken;

thread_local! {
    static CURRENT_SOFTWARE_CPU: RefCell<Option<Arc<SoftwareCpuToken>>> = const { RefCell::new(None) };
}

struct ActiveCpu {
    previous: Option<Arc<SoftwareCpuToken>>,
}

impl ActiveCpu {
    fn enter(token: Arc<SoftwareCpuToken>) -> Self {
        let previous = CURRENT_SOFTWARE_CPU.with(|current| {
            // Install this poll's dynamic software CPU token.
            current.replace(Some(token))
        });
        Self { previous }
    }
}

impl Drop for ActiveCpu {
    fn drop(&mut self) {
        CURRENT_SOFTWARE_CPU.with(|current| {
            // Restore the enclosing poll's token, if one exists.
            current.replace(self.previous.take());
        });
    }
}

fn current_software_cpu() -> Option<Arc<SoftwareCpuToken>> {
    CURRENT_SOFTWARE_CPU.with(|current| current.borrow().clone())
}

/// Single modeled CPU token shared by SW main and IRQ handler contexts.
#[derive(Debug, Clone)]
pub struct SoftwareCpu {
    gate: Arc<Mutex<()>>,
    token: Arc<SoftwareCpuToken>,
}

type CpuAcquisition = Pin<Box<dyn Future<Output = OwnedMutexGuard<()>> + Send>>;

impl SoftwareCpu {
    /// Creates one idle modeled CPU.
    pub fn new() -> Self {
        Self {
            gate: Arc::new(Mutex::new(())),
            token: Arc::new(SoftwareCpuToken),
        }
    }

    /// Binds one mutable value to this modeled CPU.
    pub fn local<T>(&self, value: T) -> CpuLocal<T> {
        CpuLocal::new(Arc::clone(&self.token), value)
    }

    /// Runs one complete SW-main program as atomic poll-to-pending segments.
    pub async fn run_main<F, T>(&self, program: F) -> T
    where
        F: Future<Output = T>,
    {
        let gate = Arc::clone(&self.gate);
        let token = Arc::clone(&self.token);
        let mut program = Box::pin(program);
        let mut acquisition: Option<CpuAcquisition> = None;
        poll_fn(move |context| {
            let acquire =
                acquisition.get_or_insert_with(|| Box::pin(Arc::clone(&gate).lock_owned()));
            let cpu = ready!(acquire.as_mut().poll(context));
            acquisition = None;
            let active = ActiveCpu::enter(Arc::clone(&token));
            let state = program.as_mut().poll(context);
            drop(active);
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
        let token = Arc::clone(&self.token);
        let mut work = Box::pin(work);
        poll_fn(move |context| {
            let active = ActiveCpu::enter(Arc::clone(&token));
            let state = work.as_mut().poll(context);
            drop(active);
            state
        })
        .await
    }
}

impl Default for SoftwareCpu {
    fn default() -> Self {
        Self::new()
    }
}
