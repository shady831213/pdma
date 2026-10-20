use super::{
    Arc, AtomicUsize, DropHandler, DropSource, HandlerRegistry, InterruptDispatcher, Notice,
    Notify, Observer, Ordering, SharedInterruptObserver, SoftwareCpu, freeze,
};
use std::{
    future::Future,
    task::{Context, Waker},
};

#[tokio::test(flavor = "current_thread")]
async fn cancelled_polled_join_preserves_accepted_handler_until_completion() {
    // Given an accepted handler suspended until its owner releases it.
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
        Arc::new(SoftwareCpu::new()),
        freeze(handlers),
        DropSource {
            delivered: false,
            drops: Arc::clone(&drops),
        },
        shared,
    );
    started.notified().await;

    // When the join has requested shutdown and its suspended future is cancelled.
    let mut join = Box::pin(task.shutdown_and_join());
    assert!(
        join.as_mut()
            .poll(&mut Context::from_waker(Waker::noop()))
            .is_pending()
    );
    drop(join);

    // Then accepted work retains its state until release and reports success.
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
