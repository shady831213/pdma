//! Demonstrates segmented main execution and an atomic pending handler turn.
//!
//! Run: `cargo run -p hw-model-swenv --example software_env_segments`.
//! This is a scheduling demonstration, not an instruction-set simulation.
//! Manual polling exposes boundaries deterministically; ordinary programs should
//! use `InterruptRuntime::run_main` and let Tokio drive their futures.

use hw_model_swenv::{EnvContextError, SoftwareEnv};
use std::{
    error::Error,
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};
use tokio::sync::oneshot;

fn poll_once<F>(future: Pin<&mut F>) -> Poll<F::Output>
where
    F: Future + ?Sized,
{
    let mut context = Context::from_waker(std::task::Waker::noop());
    future.poll(&mut context)
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn Error>> {
    let env = SoftwareEnv::new();
    // Only explicitly environment-local state requires this execution context. Ordinary
    // Rust driver state does not become environment-bound by being used in a program.
    let events = env.local(Vec::<&'static str>::new());
    let main_events = events.clone();
    let handler_events = events.clone();
    let (resume_main_tx, resume_main_rx) = oneshot::channel();
    let (resume_handler_tx, resume_handler_rx) = oneshot::channel();
    let mut main = Box::pin(env.run_main(async move {
        main_events.with(|events| events.push("main-before"))?;
        let _ = resume_main_rx.await;
        main_events.with(|events| events.push("main-after"))
    }));
    if poll_once(main.as_mut()) != Poll::Pending {
        return Err("main did not expose a real Pending segment".into());
    }
    // A real Pending releases main's turn. The handler holds its turn across
    // its own awaits, so waking main cannot interleave it with this ISR.
    let mut handler = Box::pin(env.run_handler_turn(async move {
        handler_events.with(|events| events.push("handler-before"))?;
        let _ = resume_handler_rx.await;
        handler_events.with(|events| events.push("handler-after"))
    }));
    if poll_once(handler.as_mut()) != Poll::Pending {
        return Err("handler did not stay pending while atomic".into());
    }
    let _ = resume_main_tx.send(());
    if poll_once(main.as_mut()) != Poll::Pending {
        return Err("main interleaved with the pending handler".into());
    }
    let _ = resume_handler_tx.send(());
    if poll_once(handler.as_mut()) != Poll::Ready(Ok(())) {
        return Err("handler did not complete after release".into());
    }
    if poll_once(main.as_mut()) != Poll::Ready(Ok(())) {
        return Err("main did not resume after the handler".into());
    }
    let actual_events = env
        .run_main(async move { events.with(|events| events.clone()) })
        .await?;
    let expected_events = [
        "main-before",
        "handler-before",
        "handler-after",
        "main-after",
    ];
    if actual_events != expected_events {
        return Err("software environment event order changed".into());
    }

    // The capability check applies on use, not just when a future is created.
    // Moving a suspended future to another modeled environment cannot bypass it.
    let inner_commits = env.local(0_usize);
    let off_context = inner_commits.with(|commits| *commits);
    let mut operation = Box::pin(async {
        tokio::task::yield_now().await;
        inner_commits.with(|commits| *commits += 1)
    });
    env.run_main(async { assert_pending(operation.as_mut()) })
        .await?;
    let wrong_env = SoftwareEnv::new();
    let wrong_repoll = wrong_env.run_main(operation.as_mut()).await;
    let commit_count = env
        .run_main(async { inner_commits.with(|commits| *commits) })
        .await?;
    if wrong_repoll != Err(EnvContextError::Mismatch)
        || off_context != Err(EnvContextError::Inactive)
        || commit_count != 0
    {
        return Err("preflight validation committed or returned the wrong error".into());
    }

    println!("events={}", actual_events.join(","));
    println!("wrong_repoll=Mismatch inner_commits={commit_count}");
    println!("off_context=Inactive");
    Ok(())
}

fn assert_pending<F>(future: Pin<&mut F>) -> Result<(), Box<dyn Error>>
where
    F: Future + ?Sized,
{
    match poll_once(future) {
        Poll::Pending => Ok(()),
        Poll::Ready(_) => Err("owner operation did not first return Pending".into()),
    }
}
