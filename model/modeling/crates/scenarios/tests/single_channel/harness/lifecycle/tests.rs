use super::run;
use crate::harness::TestResult;
use std::{
    future::{self, Future},
    io,
    sync::Arc,
};
use tokio::{sync::oneshot, task::JoinError};

async fn bounded<T>(execution: impl Future<Output = T>) -> TestResult<T> {
    tokio::select! {
        biased;
        result = execution => Ok(result),
        () = async { for _ in 0..30_000 { tokio::task::yield_now().await; } } => {
            Err("lifecycle test exhausted its scheduling budget".into())
        }
    }
}

#[tokio::test(flavor = "current_thread")]
async fn software_completion_joins_worker_cancellation() -> TestResult {
    // Given: a persistent worker owning the last strong reference to a resource.
    let resource = Arc::new(());
    let released = Arc::downgrade(&resource);
    let worker = async move {
        let result = future::pending::<TestResult>().await;
        drop(resource);
        result
    };
    // When: software finishes while hardware is still pending.
    let (value, joined) = bounded(run(worker, async { Ok(7) })).await??;
    // Then: cancellation was joined and the worker released its owned resource.
    assert_eq!(value, 7);
    assert!(joined.is_cancelled());
    assert!(released.upgrade().is_none());
    Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn software_failure_joins_worker_before_returning_error() -> TestResult {
    // Given: hardware retains a resource until its future is destroyed.
    let resource = Arc::new(());
    let released = Arc::downgrade(&resource);
    let worker = async move {
        let result = future::pending::<TestResult>().await;
        drop(resource);
        result
    };
    // When: the software runtime fails before producing an outcome.
    let result = bounded(run(worker, async {
        Err::<(), _>(io::Error::from(io::ErrorKind::InvalidData).into())
    }))
    .await?;
    // Then: the original error returns only after worker ownership is released.
    let error = result.err().ok_or("software failure was lost")?;
    assert_eq!(
        error.downcast_ref::<io::Error>().map(io::Error::kind),
        Some(io::ErrorKind::InvalidData)
    );
    assert!(released.upgrade().is_none());
    Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn worker_error_wins_over_ready_software_completion() -> TestResult {
    // Given: a worker makes software ready immediately before returning its error.
    let (ready, software_ready) = oneshot::channel();
    let worker = async move {
        ready.send(()).map_err(|()| "software dropped readiness")?;
        Err(io::Error::from(io::ErrorKind::BrokenPipe).into())
    };
    // When: both outcomes can be observed by the harness.
    let result = bounded(run(worker, async {
        software_ready.await?;
        Ok(())
    }))
    .await?;
    // Then: the hardware failure cannot be mistaken for requested cancellation.
    let error = result.err().ok_or("worker failure was lost")?;
    assert_eq!(
        error.downcast_ref::<io::Error>().map(io::Error::kind),
        Some(io::ErrorKind::BrokenPipe)
    );
    Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn worker_success_terminates_waiting_software_with_failure() -> TestResult {
    // Given: an accidentally finite hardware worker and software waiting forever.
    let worker = async { Ok(()) };
    // When: the worker returns successfully without a shutdown request.
    let result = bounded(run(worker, future::pending::<TestResult>())).await?;
    // Then: infrastructure reports unexpected termination instead of waiting forever.
    let error = result
        .err()
        .ok_or("unexpected worker completion was lost")?;
    assert_eq!(
        error.downcast_ref::<io::Error>().map(io::Error::kind),
        Some(io::ErrorKind::UnexpectedEof)
    );
    Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn worker_panic_is_reported_as_join_failure() -> TestResult {
    // Given: hardware unwinds while software remains pending.
    let worker = async { std::panic::resume_unwind(Box::new("worker panic")) };
    // When: the harness observes the worker's termination.
    let result = bounded(run(worker, future::pending::<TestResult>())).await?;
    // Then: the original panic is visible through its join error.
    let error = result.err().ok_or("worker panic was lost")?;
    assert!(
        error
            .downcast_ref::<JoinError>()
            .is_some_and(JoinError::is_panic)
    );
    Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn scheduling_budget_joins_stalled_worker() -> TestResult {
    // Given: both actors remain pending and hardware owns a resource.
    let resource = Arc::new(());
    let released = Arc::downgrade(&resource);
    let worker = async move {
        let result = future::pending::<TestResult>().await;
        drop(resource);
        result
    };
    // When: the scenario's finite scheduling budget is consumed.
    let result = bounded(run(worker, future::pending::<TestResult>())).await?;
    // Then: the bounded failure includes completed worker teardown.
    let error = result.err().ok_or("stalled scenario completed")?;
    assert_eq!(
        error.downcast_ref::<io::Error>().map(io::Error::kind),
        Some(io::ErrorKind::TimedOut)
    );
    assert!(released.upgrade().is_none());
    Ok(())
}
