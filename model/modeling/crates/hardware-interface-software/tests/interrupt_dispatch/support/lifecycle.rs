use super::{Notice, Observer, RecordingHandler, ToyError, freeze, noop, registry};
use hardware_interface_irq::{InterruptHandler, InterruptSource};
use hardware_interface_software::{
    HandlerRegistry, InterruptDispatcher, InterruptShutdown, SharedInterruptObserver, SoftwareEnv,
};
use std::{
    convert::Infallible,
    future::Future,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
};
use tokio::sync::Notify;

mod cancellation;

struct PostReadySource {
    shutdown: Arc<Mutex<Option<InterruptShutdown>>>,
    release: Arc<Notify>,
    claims: Arc<AtomicUsize>,
    returns: Arc<AtomicUsize>,
}

#[tokio::test(flavor = "current_thread")]
async fn accepted_notification_waiting_for_env_ignores_shutdown_until_handler_completes() {
    let env = Arc::new(SoftwareEnv::new());
    let (env_release_tx, env_release_rx) = tokio::sync::oneshot::channel();
    let holder_env = Arc::clone(&env);
    let holder = tokio::spawn(async move {
        holder_env
            .run_handler_turn(async move {
                let _ = env_release_rx.await;
            })
            .await;
    });
    tokio::task::yield_now().await;
    let calls = Arc::new(Mutex::new(Vec::new()));
    let source = super::ToySource::new([Ok(Some(Notice {
        vector: 1,
        value: 6,
    }))]);
    let task = InterruptDispatcher::start(
        env,
        registry(RecordingHandler {
            calls: Arc::clone(&calls),
            release: None,
            fail: false,
        }),
        source.clone(),
        noop(),
    );
    while source.consumed() == 0 {
        tokio::task::yield_now().await;
    }
    let join = tokio::spawn(task.shutdown_and_join());
    tokio::task::yield_now().await;
    assert!(!join.is_finished());
    let _ = env_release_tx.send(());
    assert!(holder.await.is_ok());
    let summary = join.await.expect("join task").expect("dispatcher joins");
    assert_eq!(summary.deliveries(), 1);
    assert_eq!(
        *calls
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner),
        [6]
    );
}

impl InterruptSource for PostReadySource {
    type Notification = Notice;
    type Ready = ();
    type Error = Infallible;

    async fn wait_ready(&self) -> Result<Self::Ready, Self::Error> {
        self.release.notified().await;
        let shutdown = self
            .shutdown
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();
        if let Some(shutdown) = shutdown {
            shutdown.request_shutdown();
        }
        self.returns.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }

    fn claim(
        &mut self,
        _ready: Self::Ready,
    ) -> impl Future<Output = Result<Option<Notice>, Self::Error>> + Send {
        self.claims.fetch_add(1, Ordering::SeqCst);
        std::future::ready(Ok(Some(Notice {
            vector: 1,
            value: 1,
        })))
    }
}

#[tokio::test(flavor = "current_thread")]
async fn post_ready_shutdown_check_prevents_claim() {
    let shutdown = Arc::new(Mutex::new(None));
    let release = Arc::new(Notify::new());
    let claims = Arc::new(AtomicUsize::new(0));
    let returns = Arc::new(AtomicUsize::new(0));
    let source = PostReadySource {
        shutdown: Arc::clone(&shutdown),
        release: Arc::clone(&release),
        claims: Arc::clone(&claims),
        returns: Arc::clone(&returns),
    };
    let task = InterruptDispatcher::start(
        Arc::new(SoftwareEnv::new()),
        registry(RecordingHandler {
            calls: Arc::new(Mutex::new(Vec::new())),
            release: None,
            fail: false,
        }),
        source,
        noop(),
    );
    *shutdown
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(task.shutdown_handle());
    release.notify_one();
    while returns.load(Ordering::SeqCst) == 0 {
        tokio::task::yield_now().await;
    }
    let summary = task.shutdown_and_join().await.expect("joins");
    assert_eq!(summary.deliveries(), 0);
    assert_eq!(claims.load(Ordering::SeqCst), 0);
}

struct DropSource {
    delivered: bool,
    drops: Arc<AtomicUsize>,
}

impl Drop for DropSource {
    fn drop(&mut self) {
        self.drops.fetch_add(1, Ordering::SeqCst);
    }
}

impl InterruptSource for DropSource {
    type Notification = Notice;
    type Ready = ();
    type Error = Infallible;

    async fn wait_ready(&self) -> Result<Self::Ready, Self::Error> {
        Ok(())
    }

    async fn claim(&mut self, _ready: Self::Ready) -> Result<Option<Notice>, Self::Error> {
        if self.delivered {
            std::future::pending().await
        } else {
            self.delivered = true;
            Ok(Some(Notice {
                vector: 1,
                value: 5,
            }))
        }
    }
}

struct DropHandler {
    started: Arc<Notify>,
    release: Arc<Notify>,
    drops: Arc<AtomicUsize>,
}

impl Drop for DropHandler {
    fn drop(&mut self) {
        self.drops.fetch_add(1, Ordering::SeqCst);
    }
}

impl InterruptHandler<Notice> for DropHandler {
    type Error = ToyError;

    async fn handle(&mut self, _notification: &Notice) -> Result<(), Self::Error> {
        self.started.notify_one();
        self.release.notified().await;
        Ok(())
    }
}

#[tokio::test(flavor = "current_thread")]
async fn dropped_owner_detaches_non_cancellable_turn_then_releases_owned_state() {
    let started = Arc::new(Notify::new());
    let release = Arc::new(Notify::new());
    let drops = Arc::new(AtomicUsize::new(0));
    let mut handlers = HandlerRegistry::new();
    assert!(
        handlers
            .register(
                1,
                DropHandler {
                    started: Arc::clone(&started),
                    release: Arc::clone(&release),
                    drops: Arc::clone(&drops),
                },
            )
            .is_ok()
    );
    let observer = Arc::new(Observer::default());
    let shared: SharedInterruptObserver<Notice> = observer.clone();
    let task = InterruptDispatcher::start(
        Arc::new(SoftwareEnv::new()),
        freeze(handlers),
        DropSource {
            delivered: false,
            drops: Arc::clone(&drops),
        },
        shared,
    );
    started.notified().await;
    drop(task);
    assert_eq!(drops.load(Ordering::SeqCst), 0);
    release.notify_one();
    while drops.load(Ordering::SeqCst) != 2 {
        tokio::task::yield_now().await;
    }
    assert_eq!(
        *observer
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner),
        ["start", "success"]
    );
}
