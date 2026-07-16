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
struct CpuIdentity;

thread_local! {
    static ACTIVE_CPUS: RefCell<Vec<Arc<CpuIdentity>>> = const { RefCell::new(Vec::new()) };
}

struct ActiveCpu;

impl ActiveCpu {
    fn enter(identity: Arc<CpuIdentity>) -> Self {
        ACTIVE_CPUS.with(|active| active.borrow_mut().push(identity));
        Self
    }
}

impl Drop for ActiveCpu {
    fn drop(&mut self) {
        ACTIVE_CPUS.with(|active| drop(active.borrow_mut().pop()));
    }
}

fn active_cpu() -> Option<Arc<CpuIdentity>> {
    ACTIVE_CPUS.with(|active| active.borrow().last().cloned())
}

/// Single modeled CPU token shared by SW main and IRQ handler contexts.
#[derive(Debug, Clone)]
pub struct SoftwareCpu {
    gate: Arc<Mutex<()>>,
    identity: Arc<CpuIdentity>,
}

type CpuAcquisition = Pin<Box<dyn Future<Output = OwnedMutexGuard<()>> + Send>>;

impl SoftwareCpu {
    /// Creates one idle modeled CPU.
    pub fn new() -> Self {
        Self {
            gate: Arc::new(Mutex::new(())),
            identity: Arc::new(CpuIdentity),
        }
    }

    /// Binds one mutable value to this modeled CPU.
    pub fn local<T>(&self, value: T) -> CpuLocal<T> {
        CpuLocal::new(Arc::clone(&self.identity), value)
    }

    /// Runs one complete SW-main program as atomic poll-to-pending segments.
    pub async fn run_main<F, T>(&self, program: F) -> T
    where
        F: Future<Output = T>,
    {
        let gate = Arc::clone(&self.gate);
        let identity = Arc::clone(&self.identity);
        let mut program = Box::pin(program);
        let mut acquisition: Option<CpuAcquisition> = None;
        poll_fn(move |context| {
            let acquire =
                acquisition.get_or_insert_with(|| Box::pin(Arc::clone(&gate).lock_owned()));
            let cpu = ready!(acquire.as_mut().poll(context));
            acquisition = None;
            let active = ActiveCpu::enter(Arc::clone(&identity));
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
        let identity = Arc::clone(&self.identity);
        let mut work = Box::pin(work);
        poll_fn(move |context| {
            let active = ActiveCpu::enter(Arc::clone(&identity));
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
