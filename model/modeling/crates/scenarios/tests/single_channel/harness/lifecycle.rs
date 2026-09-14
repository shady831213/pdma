//! Persistent hardware supervision and joined teardown for scenario software.

mod tests;

use super::TestResult;
use std::{error::Error, future::Future, io};
use tokio::task::{JoinError, JoinSet};

/// Runs software beside hardware and returns its joined cancellation receipt.
pub async fn run<T>(
    worker: impl Future<Output = TestResult> + Send + 'static,
    program: impl Future<Output = TestResult<T>>,
) -> TestResult<(T, JoinError)> {
    let mut workers = JoinSet::new();
    workers.spawn(worker);
    let result = tokio::select! {
        biased;
        joined = workers.join_next() => return Err(worker_failure(joined)),
        result = program => result,
        () = async { for _ in 0..10_000 { tokio::task::yield_now().await; } } => {
            Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "single-channel scenario exhausted its scheduling budget",
            ).into())
        }
    };
    workers.abort_all();
    let joined = match workers.join_next().await {
        Some(Err(error)) if error.is_cancelled() => error,
        other => return Err(worker_failure(other)),
    };
    Ok((result?, joined))
}

fn worker_failure(joined: Option<Result<TestResult, JoinError>>) -> Box<dyn Error + Send + Sync> {
    match joined {
        Some(Ok(Err(error))) => error,
        Some(Ok(Ok(()))) => io::Error::new(
            io::ErrorKind::UnexpectedEof,
            "persistent hardware worker unexpectedly completed",
        )
        .into(),
        Some(Err(error)) => error.into(),
        None => "hardware worker is missing".into(),
    }
}
