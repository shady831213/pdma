use super::error::{ConsumerError, EnvCounts, lock};
use hw_model_irq::{InterruptEnvelope, InterruptHandler, InterruptSource};
use hir::{
    EnvLocal, InterruptLifecycleEvent, InterruptLifecycleObserver, SharedInterruptObserver,
    TraceBuffer,
};
use std::{
    collections::VecDeque,
    convert::Infallible,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
};
use tokio::sync::oneshot;

mod adverse;
mod happy;

pub(super) async fn run_happy_dispatch(
    trace: &TraceBuffer<&'static str>,
) -> Result<usize, ConsumerError> {
    happy::run_happy_dispatch(trace).await
}

pub(super) async fn run_adverse_dispatch() -> Result<(), ConsumerError> {
    adverse::run_adverse_dispatch().await
}

#[derive(Clone, Debug)]
pub(super) struct Notice(pub u8);

impl InterruptEnvelope for Notice {
    type Vector = u8;

    fn vector(&self) -> Self::Vector {
        self.0
    }
}

#[derive(Debug, Default)]
pub(super) struct SourceState {
    notices: VecDeque<Notice>,
    pub(super) consumed: usize,
    pub(super) claimed: Option<oneshot::Sender<()>>,
}

#[derive(Debug)]
pub(super) struct Source {
    state: Arc<Mutex<SourceState>>,
    drops: Arc<AtomicUsize>,
}

impl Source {
    pub(super) fn one(vector: u8, state: Arc<Mutex<SourceState>>, drops: Arc<AtomicUsize>) -> Self {
        lock(&state).notices.push_back(Notice(vector));
        Self { state, drops }
    }
}

impl Drop for Source {
    fn drop(&mut self) {
        self.drops.fetch_add(1, Ordering::SeqCst);
    }
}

impl InterruptSource for Source {
    type Notification = Notice;
    type Ready = ();
    type Error = Infallible;

    async fn wait_ready(&self) -> Result<Self::Ready, Self::Error> {
        if lock(&self.state).notices.is_empty() {
            std::future::pending::<Result<(), Infallible>>().await
        } else {
            Ok(())
        }
    }

    async fn claim(&mut self, _ready: Self::Ready) -> Result<Option<Notice>, Self::Error> {
        let notice = lock(&self.state).notices.pop_front();
        if notice.is_some() {
            let mut state = lock(&self.state);
            state.consumed = state.consumed.saturating_add(1);
            if let Some(claimed) = state.claimed.take() {
                let _ = claimed.send(());
            }
        }
        Ok(notice)
    }
}

#[derive(Debug)]
pub(super) struct Handler {
    pub(super) entries: Arc<AtomicUsize>,
    env_state: Option<EnvLocal<EnvCounts>>,
    release: Option<oneshot::Receiver<()>>,
    started: Option<oneshot::Sender<()>>,
    drops: Arc<AtomicUsize>,
}

impl Handler {
    pub(super) const fn pending(
        entries: Arc<AtomicUsize>,
        env_state: EnvLocal<EnvCounts>,
        release: oneshot::Receiver<()>,
        started: oneshot::Sender<()>,
        drops: Arc<AtomicUsize>,
    ) -> Self {
        Self {
            entries,
            env_state: Some(env_state),
            release: Some(release),
            started: Some(started),
            drops,
        }
    }

    pub(super) const fn immediate(entries: Arc<AtomicUsize>, drops: Arc<AtomicUsize>) -> Self {
        Self {
            entries,
            env_state: None,
            release: None,
            started: None,
            drops,
        }
    }
}

impl Drop for Handler {
    fn drop(&mut self) {
        self.drops.fetch_add(1, Ordering::SeqCst);
    }
}

impl InterruptHandler<Notice> for Handler {
    type Error = ConsumerError;

    async fn handle(&mut self, _notification: &Notice) -> Result<(), Self::Error> {
        self.entries.fetch_add(1, Ordering::SeqCst);
        if let Some(env_state) = &self.env_state {
            env_state.with(|state| {
                state.handler_entries = state.handler_entries.saturating_add(1);
            })?;
        }
        if let Some(started) = self.started.take() {
            let _ = started.send(());
        }
        if let Some(release) = self.release.take() {
            let _ = release.await;
        }
        Ok(())
    }
}

#[derive(Debug)]
pub(super) struct Lifecycle {
    pub(super) events: Arc<AtomicUsize>,
    pub(super) trace: TraceBuffer<&'static str>,
}

impl Lifecycle {
    pub(super) fn observer(self) -> SharedInterruptObserver<Notice> {
        Arc::new(self)
    }
}

impl InterruptLifecycleObserver<Notice> for Lifecycle {
    fn observe(&self, event: InterruptLifecycleEvent<'_, Notice>) {
        let label = match event {
            InterruptLifecycleEvent::Started { .. } => "handler-start",
            InterruptLifecycleEvent::Succeeded { .. } => "handler-finish",
            InterruptLifecycleEvent::Failed { .. } => "handler-failed",
        };
        self.events.fetch_add(1, Ordering::SeqCst);
        self.trace.push(label);
    }
}
