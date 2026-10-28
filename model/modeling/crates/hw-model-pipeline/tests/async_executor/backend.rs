use std::{
    future::pending,
    sync::{
        Mutex, MutexGuard, PoisonError,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
};

use hw_model_pipeline::{AsyncInteractionBackend, StepResult};

use super::Context;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum BackendFault {
    Before,
    After,
}

pub(super) struct ScriptedBackend {
    pub(super) context: Mutex<Context>,
    before_open: AtomicBool,
    pub(super) after_open: AtomicBool,
    pub(super) interactions: AtomicUsize,
    failure: Option<BackendFault>,
}

impl ScriptedBackend {
    pub(super) const fn configured(
        before: bool,
        after: bool,
        failure: Option<BackendFault>,
    ) -> Self {
        Self {
            context: Mutex::new(Vec::new()),
            before_open: AtomicBool::new(before),
            after_open: AtomicBool::new(after),
            interactions: AtomicUsize::new(0),
            failure,
        }
    }

    pub(super) const fn failing(fault: BackendFault) -> Self {
        Self::configured(true, true, Some(fault))
    }
}

impl AsyncInteractionBackend<Context> for ScriptedBackend {
    type Error = BackendFault;

    async fn yield_now(&self) {
        tokio::task::yield_now().await;
    }

    async fn interact<O: Send, E: Send, F>(
        &self,
        interaction: F,
    ) -> Result<StepResult<O, E>, Self::Error>
    where
        F: FnOnce(&mut Context) -> StepResult<O, E> + Send,
    {
        if self.failure == Some(BackendFault::Before) {
            return Err(BackendFault::Before);
        }
        if !self.before_open.load(Ordering::SeqCst) {
            pending::<()>().await;
        }
        let outcome = {
            self.interactions.fetch_add(1, Ordering::SeqCst);
            interaction(&mut lock(&self.context))
        };
        if !self.after_open.load(Ordering::SeqCst) {
            pending::<()>().await;
        }
        match self.failure {
            Some(BackendFault::After) => Err(BackendFault::After),
            Some(BackendFault::Before) | None => Ok(outcome),
        }
    }
}

pub(super) fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}
