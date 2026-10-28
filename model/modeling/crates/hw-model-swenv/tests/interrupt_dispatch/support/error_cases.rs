use super::{Notice, Observer, RecordingHandler, ToyError, ToySource, noop, registry};
use hw_model_swenv::{
    InterruptDispatchError, InterruptDispatcher, InterruptJoinError, SharedInterruptObserver,
    SoftwareEnv,
};
use std::{
    error::Error,
    sync::{Arc, Mutex},
};
use tokio::sync::oneshot;

#[tokio::test(flavor = "current_thread")]
async fn readiness_and_claim_errors_preserve_typed_sources() {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let ready_source = ToySource::readiness_error();
    let ready_task = InterruptDispatcher::start(
        Arc::new(SoftwareEnv::new()),
        registry(RecordingHandler {
            calls: Arc::clone(&calls),
            release: None,
            fail: false,
        }),
        ready_source.clone(),
        noop(),
    );
    while ready_source.lock().wait_calls == 0 {
        tokio::task::yield_now().await;
    }
    let ready_error = ready_task
        .shutdown_and_join()
        .await
        .expect_err("ready fails");
    assert_eq!(
        ready_error
            .source()
            .and_then(Error::source)
            .and_then(|source| source.downcast_ref::<ToyError>())
            .map(|source| source.0),
        Some("ready")
    );
    assert!(matches!(
        ready_error,
        InterruptJoinError::Dispatch(InterruptDispatchError::Readiness(ToyError("ready")))
    ));
    let claim_source = ToySource::new([Err(ToyError("claim"))]);
    let claim_task = InterruptDispatcher::start(
        Arc::new(SoftwareEnv::new()),
        registry(RecordingHandler {
            calls,
            release: None,
            fail: false,
        }),
        claim_source.clone(),
        noop(),
    );
    while claim_source.lock().claim_calls == 0 {
        tokio::task::yield_now().await;
    }
    let claim_error = claim_task
        .shutdown_and_join()
        .await
        .expect_err("claim fails");
    assert_eq!(
        claim_error
            .source()
            .and_then(Error::source)
            .and_then(|source| source.downcast_ref::<ToyError>())
            .map(|source| source.0),
        Some("claim")
    );
    assert!(matches!(
        claim_error,
        InterruptJoinError::Dispatch(InterruptDispatchError::Claim(ToyError("claim")))
    ));
}

#[tokio::test(flavor = "current_thread")]
async fn unregistered_lookup_finishes_before_env_or_observer() {
    let env = Arc::new(SoftwareEnv::new());
    let (release_tx, release_rx) = oneshot::channel();
    let holder_env = Arc::clone(&env);
    let holder = tokio::spawn(async move {
        holder_env
            .run_handler_turn(async move {
                let _ = release_rx.await;
            })
            .await;
    });
    tokio::task::yield_now().await;
    let source = ToySource::new([Ok(Some(Notice {
        vector: 2,
        value: 9,
    }))]);
    let observer = Arc::new(Observer::default());
    let shared: SharedInterruptObserver<Notice> = observer.clone();
    let task = InterruptDispatcher::start(
        env,
        registry(RecordingHandler {
            calls: Arc::new(Mutex::new(Vec::new())),
            release: None,
            fail: false,
        }),
        source.clone(),
        shared,
    );
    while source.consumed() == 0 {
        tokio::task::yield_now().await;
    }
    let error = task.shutdown_and_join().await.expect_err("unregistered");
    assert!(matches!(
        error,
        InterruptJoinError::Dispatch(InterruptDispatchError::UnregisteredVector(2))
    ));
    assert!(
        observer
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .is_empty()
    );
    let _ = release_tx.send(());
    assert!(holder.await.is_ok());
}

#[tokio::test(flavor = "current_thread")]
async fn accepted_pending_handler_is_non_cancellable_and_reports_error_order() {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let source = ToySource::new([Ok(Some(Notice {
        vector: 1,
        value: 8,
    }))]);
    let observer = Arc::new(Observer::default());
    let shared: SharedInterruptObserver<Notice> = observer.clone();
    let (release_tx, release_rx) = oneshot::channel();
    let task = InterruptDispatcher::start(
        Arc::new(SoftwareEnv::new()),
        registry(RecordingHandler {
            calls: Arc::clone(&calls),
            release: Some(release_rx),
            fail: true,
        }),
        source,
        shared,
    );
    while observer
        .0
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .is_empty()
    {
        tokio::task::yield_now().await;
    }
    let join = tokio::spawn(task.shutdown_and_join());
    tokio::task::yield_now().await;
    assert!(!join.is_finished());
    let _ = release_tx.send(());
    let error = join.await.expect("join task").expect_err("handler fails");
    assert_eq!(
        error
            .source()
            .and_then(Error::source)
            .and_then(|source| source.downcast_ref::<ToyError>())
            .map(|source| source.0),
        Some("handler")
    );
    assert!(matches!(
        error,
        InterruptJoinError::Dispatch(InterruptDispatchError::Handler {
            vector: 1,
            source: ToyError("handler")
        })
    ));
    assert_eq!(
        *calls
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner),
        [8]
    );
    assert_eq!(
        *observer
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner),
        ["start", "error"]
    );
}
