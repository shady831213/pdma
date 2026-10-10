use std::{
    future::pending,
    sync::{
        Mutex, MutexGuard, PoisonError,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
};

use hardware_interface_pipeline::AsyncInteractionBackend;

use super::Context;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum BackendFault {
    Before,
    After,
    Wait,
}

pub(super) struct ScriptedBackend {
    pub(super) context: Mutex<Context>,
    before_open: AtomicBool,
    pub(super) after_open: AtomicBool,
    pub(super) interactions: AtomicUsize,
    pub(super) generation: AtomicUsize,
    pub(super) wait_checks: AtomicUsize,
    pub(super) bump_after: AtomicBool,
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
            generation: AtomicUsize::new(0),
            wait_checks: AtomicUsize::new(0),
            bump_after: AtomicBool::new(false),
            failure,
        }
    }

    pub(super) const fn failing(fault: BackendFault) -> Self {
        Self::configured(true, true, Some(fault))
    }
}

impl AsyncInteractionBackend<Context, usize> for ScriptedBackend {
    type Error = BackendFault;

    async fn interact<R: Send, F>(&self, interaction: F) -> Result<R, Self::Error>
    where
        F: FnOnce(&mut Context) -> R + Send,
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
        if self.bump_after.swap(false, Ordering::SeqCst) {
            self.generation.fetch_add(1, Ordering::SeqCst);
        }
        if !self.after_open.load(Ordering::SeqCst) {
            pending::<()>().await;
        }
        match self.failure {
            Some(BackendFault::After) => Err(BackendFault::After),
            Some(BackendFault::Before | BackendFault::Wait) | None => Ok(outcome),
        }
    }

    async fn wait(&self, wait: usize) -> Result<(), Self::Error> {
        if self.failure == Some(BackendFault::Wait) {
            return Err(BackendFault::Wait);
        }
        self.wait_checks.fetch_add(1, Ordering::SeqCst);
        if self.generation.load(Ordering::SeqCst) == wait {
            pending::<()>().await;
        }
        Ok(())
    }
}

pub(super) fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}
