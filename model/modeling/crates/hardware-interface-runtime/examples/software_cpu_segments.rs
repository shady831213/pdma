//! Demonstrates segmented main execution and an atomic pending handler turn.

use hardware_interface_runtime::{CpuContextError, SoftwareCpu};
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
    let cpu = SoftwareCpu::new();
    let events = cpu.local(Vec::<&'static str>::new());
    let main_events = events.clone();
    let handler_events = events.clone();
    let (resume_main_tx, resume_main_rx) = oneshot::channel();
    let (resume_handler_tx, resume_handler_rx) = oneshot::channel();
    let mut main = Box::pin(cpu.run_main(async move {
        main_events.with(|events| events.push("main-before"))?;
        let _ = resume_main_rx.await;
        main_events.with(|events| events.push("main-after"))
    }));
    if poll_once(main.as_mut()) != Poll::Pending {
        return Err("main did not expose a real Pending segment".into());
    }
    let mut handler = Box::pin(cpu.run_handler_turn(async move {
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
    let actual_events = cpu
        .run_main(async move { events.with(|events| events.clone()) })
        .await?;
    let expected_events = [
        "main-before",
        "handler-before",
        "handler-after",
        "main-after",
    ];
    if actual_events != expected_events {
        return Err("software CPU event order changed".into());
    }

    let inner_commits = cpu.local(0_usize);
    let off_context = inner_commits.with(|commits| *commits);
    let mut operation = Box::pin(async {
        tokio::task::yield_now().await;
        inner_commits.with(|commits| *commits += 1)
    });
    cpu.run_main(async { assert_pending(operation.as_mut()) })
        .await?;
    let wrong_cpu = SoftwareCpu::new();
    let wrong_repoll = wrong_cpu.run_main(operation.as_mut()).await;
    let commit_count = cpu
        .run_main(async { inner_commits.with(|commits| *commits) })
        .await?;
    if wrong_repoll != Err(CpuContextError::Mismatch)
        || off_context != Err(CpuContextError::Inactive)
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
