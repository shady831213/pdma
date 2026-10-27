use hardware_interface_irq::{InterruptEnvelope, InterruptHandler, InterruptSource};
use hardware_interface_software::{
    FrozenHandlerRegistry, HandlerRegistry, InterruptLifecycleEvent, InterruptLifecycleObserver,
    TraceBuffer,
};
use std::{
    collections::VecDeque,
    convert::Infallible,
    error::Error,
    sync::{
        Arc, Mutex, MutexGuard,
        atomic::{AtomicUsize, Ordering},
    },
};
use tokio::sync::Notify;

#[derive(Debug, Clone)]
/// Toy vector-bearing notification.
pub struct Notice(pub u8);

#[derive(Debug)]
/// Typed example construction failure.
pub struct ExampleError(&'static str);

impl std::fmt::Display for ExampleError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.0)
    }
}

impl Error for ExampleError {}

impl InterruptEnvelope for Notice {
    type Vector = u8;

    fn vector(&self) -> Self::Vector {
        self.0
    }
}

#[derive(Debug)]
struct SourceState {
    notices: VecDeque<Notice>,
    consumed: usize,
}

/// Cloneable toy two-phase source with a release sentinel.
#[derive(Debug, Clone)]
pub struct Source {
    state: Arc<Mutex<SourceState>>,
    log: Arc<Mutex<Vec<String>>>,
    wake: Arc<Notify>,
    /// Counts dropped source capabilities.
    pub drops: Arc<AtomicUsize>,
}

impl Source {
    /// Creates a source with the supplied ordered notifications.
    pub fn new(notices: impl IntoIterator<Item = Notice>, log: Arc<Mutex<Vec<String>>>) -> Self {
        Self {
            state: Arc::new(Mutex::new(SourceState {
                notices: notices.into_iter().collect(),
                consumed: 0,
            })),
            log,
            wake: Arc::new(Notify::new()),
            drops: Arc::new(AtomicUsize::new(0)),
        }
    }

    fn state(&self) -> MutexGuard<'_, SourceState> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// Returns the number of successfully claimed notifications.
    pub fn consumed(&self) -> usize {
        self.state().consumed
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
        loop {
            if !self.state().notices.is_empty() {
                return Ok(());
            }
            self.wake.notified().await;
        }
    }

    async fn claim(&mut self, _ready: Self::Ready) -> Result<Option<Notice>, Self::Error> {
        let notice = self.state().notices.pop_front();
        if let Some(notice) = &notice {
            let mut state = self.state();
            state.consumed = state.consumed.saturating_add(1);
            drop(state);
            self.log
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .push(format!("claim-v{}", notice.0));
        }
        Ok(notice)
    }
}

/// Persistent toy handler with a release sentinel.
#[derive(Debug)]
pub struct Handler {
    log: Arc<Mutex<Vec<String>>>,
    pending: bool,
    entries: Arc<Mutex<usize>>,
    drops: Arc<AtomicUsize>,
}

impl Drop for Handler {
    fn drop(&mut self) {
        self.drops.fetch_add(1, Ordering::SeqCst);
    }
}

impl InterruptHandler<Notice> for Handler {
    type Error = Infallible;

    async fn handle(&mut self, notification: &Notice) -> Result<(), Self::Error> {
        {
            let mut entries = self
                .entries
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            *entries = entries.saturating_add(1);
        }
        if self.pending {
            self.log
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .push(format!("pending-v{}", notification.0));
            tokio::task::yield_now().await;
        }
        Ok(())
    }
}

/// Synchronous lifecycle observer recording order and trace sequence.
#[derive(Debug)]
pub struct Observer {
    /// Shared exact-order transcript.
    pub log: Arc<Mutex<Vec<String>>>,
    /// Clone-shared lifecycle trace.
    pub trace: TraceBuffer<(&'static str, u8)>,
}

impl InterruptLifecycleObserver<Notice> for Observer {
    fn observe(&self, event: InterruptLifecycleEvent<'_, Notice>) {
        let (label, vector) = match event {
            InterruptLifecycleEvent::Started { vector, .. } => ("start", *vector),
            InterruptLifecycleEvent::Succeeded { vector, .. } => ("finish", *vector),
            InterruptLifecycleEvent::Failed { vector, .. } => ("error", *vector),
        };
        self.log
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(format!("{label}-v{vector}"));
        self.trace.push((label, vector));
    }
}

/// Builds and freezes the two-vector example registry.
pub fn registry(
    log: Arc<Mutex<Vec<String>>>,
    entries: Arc<Mutex<usize>>,
    drops: Arc<AtomicUsize>,
) -> Result<FrozenHandlerRegistry<u8, Notice, Handler>, ExampleError> {
    let mut registry = HandlerRegistry::new();
    let first = Handler {
        log: Arc::clone(&log),
        pending: true,
        entries: Arc::clone(&entries),
        drops: Arc::clone(&drops),
    };
    let second = Handler {
        log,
        pending: false,
        entries,
        drops,
    };
    registry
        .register(1, first)
        .map_err(|_| ExampleError("duplicate first vector"))?;
    registry
        .register(2, second)
        .map_err(|_| ExampleError("duplicate second vector"))?;
    registry
        .freeze()
        .map_err(|_| ExampleError("empty registry"))
}
