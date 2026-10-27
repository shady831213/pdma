use super::{
    context::{ActiveEnv, EnvContextError, EnvOwner, EnvToken},
    local::EnvLocal,
};
use std::{
    future::{Future, poll_fn},
    pin::Pin,
    sync::Arc,
    task::ready,
};
use tokio::sync::{Mutex, OwnedMutexGuard};

type EnvAcquisition = Pin<Box<dyn Future<Output = OwnedMutexGuard<()>> + Send>>;

/// One modeled software environment shared by segmented main and atomic handler turns.
///
/// This is a logical execution context, not an instruction-set emulator, OS thread,
/// or replacement for Tokio's executor. Clones share the same scheduling gate and
/// identity. Main yields the gate on each `Poll::Pending`; a handler retains it
/// until its entire future completes, even while awaiting external work.
///
/// Use [`crate::InterruptRuntime::run_main`] for a program with registered ISRs.
/// The methods here are the lower-level scheduling boundary. Only explicit
/// [`EnvLocal`] and environment-bound register capabilities impose identity checks;
/// ordinary driver objects remain ordinary Rust values.
#[derive(Debug, Clone)]
pub struct SoftwareEnv {
    gate: Arc<Mutex<()>>,
    token: Arc<EnvToken>,
}

impl SoftwareEnv {
    /// Creates one idle software environment.
    pub fn new() -> Self {
        Self {
            gate: Arc::new(Mutex::new(())),
            token: Arc::new(EnvToken),
        }
    }

    /// Binds one mutable value to this environment.
    pub fn local<T>(&self, value: T) -> EnvLocal<T> {
        EnvLocal::new(EnvOwner::from_token(Arc::clone(&self.token)), value)
    }

    pub(crate) fn validate_active(&self) -> Result<(), EnvContextError> {
        self.owner().validate_active()
    }

    pub(super) fn owner(&self) -> EnvOwner {
        EnvOwner::from_token(Arc::clone(&self.token))
    }

    /// Runs main as FIFO-acquired atomic poll-to-pending segments.
    ///
    /// Dropping the returned future drops the program and releases any held turn.
    /// It does not roll back program effects. Do not recursively await another
    /// turn on this same context while holding a handler turn.
    pub async fn run_main<F, T>(&self, program: F) -> T
    where
        F: Future<Output = T>,
    {
        let gate = Arc::clone(&self.gate);
        let token = Arc::clone(&self.token);
        let mut program = Box::pin(program);
        let mut acquisition: Option<EnvAcquisition> = None;
        poll_fn(move |context| {
            let acquire =
                acquisition.get_or_insert_with(|| Box::pin(Arc::clone(&gate).lock_owned()));
            let env = ready!(acquire.as_mut().poll(context));
            acquisition = None;
            let active = ActiveEnv::enter(Arc::clone(&token));
            let state = program.as_mut().poll(context);
            drop(active);
            drop(env);
            state
        })
        .await
    }

    /// Runs one complete handler turn while holding the FIFO environment gate.
    ///
    /// Awaited work must be able to finish without main or another handler on this
    /// context: those cannot run until this turn ends. "Atomic" describes software
    /// scheduling, not a memory transaction. Dropping the future releases the turn
    /// without undoing effects already performed by the handler.
    pub async fn run_handler_turn<F, T>(&self, handler: F) -> T
    where
        F: Future<Output = T>,
    {
        let _env = Arc::clone(&self.gate).lock_owned().await;
        let token = Arc::clone(&self.token);
        let mut handler = Box::pin(handler);
        poll_fn(move |context| {
            let active = ActiveEnv::enter(Arc::clone(&token));
            let state = handler.as_mut().poll(context);
            drop(active);
            state
        })
        .await
    }
}

impl Default for SoftwareEnv {
    fn default() -> Self {
        Self::new()
    }
}
