use super::{
    context::{ActiveCpu, CpuContextError, CpuOwner, CpuToken},
    local::CpuLocal,
};
use std::{
    future::{Future, poll_fn},
    pin::Pin,
    sync::Arc,
    task::ready,
};
use tokio::sync::{Mutex, OwnedMutexGuard};

type CpuAcquisition = Pin<Box<dyn Future<Output = OwnedMutexGuard<()>> + Send>>;

/// One modeled software CPU shared by segmented main and atomic handler turns.
#[derive(Debug, Clone)]
pub struct SoftwareCpu {
    gate: Arc<Mutex<()>>,
    token: Arc<CpuToken>,
}

impl SoftwareCpu {
    /// Creates one idle software CPU.
    pub fn new() -> Self {
        Self {
            gate: Arc::new(Mutex::new(())),
            token: Arc::new(CpuToken),
        }
    }

    /// Binds one mutable value to this CPU.
    pub fn local<T>(&self, value: T) -> CpuLocal<T> {
        CpuLocal::new(CpuOwner::from_token(Arc::clone(&self.token)), value)
    }

    pub(crate) fn validate_active(&self) -> Result<(), CpuContextError> {
        self.owner().validate_active()
    }

    pub(super) fn owner(&self) -> CpuOwner {
        CpuOwner::from_token(Arc::clone(&self.token))
    }

    /// Runs main as FIFO-acquired atomic poll-to-pending segments.
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

    /// Runs one complete handler turn while holding the FIFO CPU gate.
    pub async fn run_handler_turn<F, T>(&self, handler: F) -> T
    where
        F: Future<Output = T>,
    {
        let _cpu = Arc::clone(&self.gate).lock_owned().await;
        let token = Arc::clone(&self.token);
        let mut handler = Box::pin(handler);
        poll_fn(move |context| {
            let active = ActiveCpu::enter(Arc::clone(&token));
            let state = handler.as_mut().poll(context);
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
