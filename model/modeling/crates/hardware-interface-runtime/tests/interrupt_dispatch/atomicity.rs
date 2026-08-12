use super::{Notice, ToyError, ToySource, freeze};
use hardware_interface_runtime::{
    HandlerRegistry, InterruptDispatcher, InterruptHandler, InterruptSource, NoopInterruptObserver,
    SoftwareCpu,
};
use std::{
    convert::Infallible,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
};
use tokio::sync::oneshot;

struct OrderedHandler {
    start: &'static str,
    finish: &'static str,
    order: Arc<Mutex<Vec<&'static str>>>,
    release: Option<oneshot::Receiver<()>>,
}

struct SerialSource {
    next: u8,
    active: Arc<AtomicUsize>,
    maximum: Arc<AtomicUsize>,
}

impl InterruptSource for SerialSource {
    type Notification = Notice;
    type Ready = ();
    type Error = Infallible;

    async fn wait_ready(&self) -> Result<Self::Ready, Self::Error> {
        if self.next < 2 {
            Ok(())
        } else {
            std::future::pending().await
        }
    }

    async fn claim(&mut self, _ready: Self::Ready) -> Result<Option<Notice>, Self::Error> {
        let active = self.active.fetch_add(1, Ordering::SeqCst).saturating_add(1);
        self.maximum.fetch_max(active, Ordering::SeqCst);
        tokio::task::yield_now().await;
        self.active.fetch_sub(1, Ordering::SeqCst);
        self.next = self.next.saturating_add(1);
        Ok(Some(Notice {
            vector: 1,
            value: self.next,
        }))
    }
}

#[tokio::test(flavor = "current_thread")]
async fn source_claims_are_strictly_serialized() {
    let active = Arc::new(AtomicUsize::new(0));
    let maximum = Arc::new(AtomicUsize::new(0));
    let calls = Arc::new(Mutex::new(Vec::new()));
    let mut handlers = HandlerRegistry::new();
    assert!(
        handlers
            .register(
                1,
                OrderedHandler {
                    start: "start",
                    finish: "finish",
                    order: Arc::clone(&calls),
                    release: None,
                }
            )
            .is_ok()
    );
    let task = InterruptDispatcher::start(
        Arc::new(SoftwareCpu::new()),
        freeze(handlers),
        SerialSource {
            next: 0,
            active: Arc::clone(&active),
            maximum: Arc::clone(&maximum),
        },
        Arc::new(NoopInterruptObserver),
    );
    while calls
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .len()
        != 4
    {
        tokio::task::yield_now().await;
    }
    let summary = task.shutdown_and_join().await.expect("joins");
    assert_eq!(summary.deliveries(), 2);
    assert_eq!(active.load(Ordering::SeqCst), 0);
    assert_eq!(maximum.load(Ordering::SeqCst), 1);
}

impl InterruptHandler<Notice> for OrderedHandler {
    type Error = ToyError;

    async fn handle(&mut self, _notification: &Notice) -> Result<(), Self::Error> {
        self.order
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(self.start);
        if let Some(release) = self.release.take() {
            let _ = release.await;
        }
        self.order
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(self.finish);
        Ok(())
    }
}

#[tokio::test(flavor = "current_thread")]
async fn pending_handler_is_atomic_against_main_and_second_notification() {
    let cpu = Arc::new(SoftwareCpu::new());
    let order = Arc::new(Mutex::new(Vec::new()));
    let (release_tx, release_rx) = oneshot::channel();
    let mut handlers = HandlerRegistry::new();
    assert!(
        handlers
            .register(
                1,
                OrderedHandler {
                    start: "a-start",
                    finish: "a-finish",
                    order: Arc::clone(&order),
                    release: Some(release_rx),
                }
            )
            .is_ok()
    );
    assert!(
        handlers
            .register(
                2,
                OrderedHandler {
                    start: "b-start",
                    finish: "b-finish",
                    order: Arc::clone(&order),
                    release: None,
                }
            )
            .is_ok()
    );
    let source = ToySource::new([
        Ok(Some(Notice {
            vector: 1,
            value: 1,
        })),
        Ok(Some(Notice {
            vector: 2,
            value: 2,
        })),
    ]);
    let task = InterruptDispatcher::start(
        Arc::clone(&cpu),
        freeze(handlers),
        source,
        Arc::new(NoopInterruptObserver),
    );
    while order
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .is_empty()
    {
        tokio::task::yield_now().await;
    }
    let main_cpu = Arc::clone(&cpu);
    let main_order = Arc::clone(&order);
    let main = tokio::spawn(async move {
        main_cpu
            .run_main(async move {
                main_order
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .push("main");
            })
            .await;
    });
    tokio::task::yield_now().await;
    assert_eq!(
        *order
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner),
        ["a-start"]
    );
    let _ = release_tx.send(());
    while order
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .len()
        != 5
    {
        tokio::task::yield_now().await;
    }
    assert!(main.await.is_ok());
    let summary = task.shutdown_and_join().await.expect("joins");
    assert_eq!(summary.deliveries(), 2);
    assert_eq!(
        *order
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner),
        ["a-start", "a-finish", "main", "b-start", "b-finish"]
    );
}
