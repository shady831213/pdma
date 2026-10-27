use hardware_interface_irq::{InterruptEnvelope, InterruptHandler, InterruptSource};
use hardware_interface_software::{
    FrozenHandlerRegistry, HandlerRegistry, InterruptLifecycleEvent, InterruptLifecycleObserver,
    NoopInterruptObserver, SharedInterruptObserver,
};
use std::{
    collections::VecDeque,
    error::Error,
    fmt,
    sync::{Arc, Mutex, MutexGuard},
};
use tokio::sync::{Notify, oneshot};

#[derive(Debug, Clone, PartialEq, Eq)]
struct Notice {
    pub(super) vector: u8,
    pub(super) value: u8,
}

impl InterruptEnvelope for Notice {
    type Vector = u8;

    fn vector(&self) -> Self::Vector {
        self.vector
    }
}

#[derive(Debug)]
struct ToyError(&'static str);

impl fmt::Display for ToyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.0)
    }
}

impl Error for ToyError {}

#[derive(Debug, Clone, Copy)]
struct Ready;

#[derive(Clone)]
struct ToySource {
    state: Arc<Mutex<ToySourceState>>,
    ready: Arc<Notify>,
    claim: Arc<Notify>,
}

struct ToySourceState {
    claims: VecDeque<Result<Option<Notice>, ToyError>>,
    readiness_error: bool,
    pub(super) gate_claim: bool,
    consumed: usize,
    pub(super) wait_calls: usize,
    pub(super) claim_calls: usize,
}

impl ToySource {
    pub(super) fn new(claims: impl IntoIterator<Item = Result<Option<Notice>, ToyError>>) -> Self {
        Self {
            state: Arc::new(Mutex::new(ToySourceState {
                claims: claims.into_iter().collect(),
                readiness_error: false,
                gate_claim: false,
                consumed: 0,
                wait_calls: 0,
                claim_calls: 0,
            })),
            ready: Arc::new(Notify::new()),
            claim: Arc::new(Notify::new()),
        }
    }

    pub(super) fn readiness_error() -> Self {
        let source = Self::new([]);
        source.lock().readiness_error = true;
        source
    }

    pub(super) fn lock(&self) -> MutexGuard<'_, ToySourceState> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    pub(super) fn consumed(&self) -> usize {
        self.lock().consumed
    }
}

impl InterruptSource for ToySource {
    type Notification = Notice;
    type Ready = Ready;
    type Error = ToyError;

    async fn wait_ready(&self) -> Result<Self::Ready, Self::Error> {
        loop {
            {
                let mut state = self.lock();
                state.wait_calls = state.wait_calls.saturating_add(1);
                if state.readiness_error {
                    return Err(ToyError("ready"));
                }
                if !state.claims.is_empty() {
                    return Ok(Ready);
                }
            }
            self.ready.notified().await;
        }
    }

    async fn claim(&mut self, _ready: Self::Ready) -> Result<Option<Notice>, Self::Error> {
        {
            let mut state = self.lock();
            state.claim_calls = state.claim_calls.saturating_add(1);
        }
        if self.lock().gate_claim {
            self.claim.notified().await;
        }
        let result = self.lock().claims.pop_front().unwrap_or(Ok(None));
        if matches!(result, Ok(Some(_))) {
            let mut state = self.lock();
            state.consumed = state.consumed.saturating_add(1);
        }
        result
    }
}

struct RecordingHandler {
    pub(super) calls: Arc<Mutex<Vec<u8>>>,
    pub(super) release: Option<oneshot::Receiver<()>>,
    pub(super) fail: bool,
}

impl InterruptHandler<Notice> for RecordingHandler {
    type Error = ToyError;

    async fn handle(&mut self, notification: &Notice) -> Result<(), Self::Error> {
        self.calls
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(notification.value);
        if let Some(release) = self.release.take() {
            let _ = release.await;
        }
        if self.fail {
            Err(ToyError("handler"))
        } else {
            Ok(())
        }
    }
}

#[derive(Default)]
struct Observer(Mutex<Vec<&'static str>>);

impl InterruptLifecycleObserver<Notice> for Observer {
    fn observe(&self, event: InterruptLifecycleEvent<'_, Notice>) {
        let label = match event {
            InterruptLifecycleEvent::Started { .. } => "start",
            InterruptLifecycleEvent::Succeeded { .. } => "success",
            InterruptLifecycleEvent::Failed { .. } => "error",
        };
        self.0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(label);
    }
}

fn registry(handler: RecordingHandler) -> FrozenHandlerRegistry<u8, Notice, RecordingHandler> {
    let mut registry = HandlerRegistry::new();
    assert!(registry.register(1, handler).is_ok());
    freeze(registry)
}

fn freeze<H>(registry: HandlerRegistry<u8, Notice, H>) -> FrozenHandlerRegistry<u8, Notice, H>
where
    H: InterruptHandler<Notice>,
{
    registry.freeze().unwrap_or_else(|_| std::process::abort())
}

fn noop() -> SharedInterruptObserver<Notice> {
    Arc::new(NoopInterruptObserver)
}

mod atomicity;
mod cases;
mod error_cases;
mod heterogeneous;
mod lifecycle;
