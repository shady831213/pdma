use super::super::SoftwareCpu;
use pdma_model_contract::ModelError;
use std::{future::Future, pin::Pin, task::Poll};
use tokio::sync::oneshot;

struct NonCloneState {
    value: usize,
}

fn poll_once<F>(future: Pin<&mut F>) -> Poll<F::Output>
where
    F: Future + ?Sized,
{
    let mut context = std::task::Context::from_waker(std::task::Waker::noop());
    future.poll(&mut context)
}

#[tokio::test(flavor = "current_thread")]
async fn cpu_local_clones_share_one_non_clone_state() {
    let cpu = SoftwareCpu::new();
    let state = cpu.local(NonCloneState { value: 1 });
    let other = state.clone();

    let result = cpu
        .run_main(async move {
            state.with(|state| state.value += 2)?;
            other.with(|state| state.value)
        })
        .await;

    assert_eq!(result, Ok(3));
}

#[test]
fn cpu_local_rejects_access_off_context() {
    let cpu = SoftwareCpu::new();
    let state = cpu.local(0_u8);

    assert_eq!(
        state.with(|value| *value),
        Err(ModelError::SoftwareCpuInactive)
    );
}

#[tokio::test(flavor = "current_thread")]
async fn cpu_local_rejects_access_from_another_cpu() {
    let owner = SoftwareCpu::new();
    let other = SoftwareCpu::new();
    let state = owner.local(0_u8);

    let result = other
        .run_main(async move { state.with(|value| *value) })
        .await;

    assert_eq!(result, Err(ModelError::SoftwareCpuMismatch));
}

#[tokio::test(flavor = "current_thread")]
async fn cpu_local_moves_between_main_handler_and_main() {
    let cpu = SoftwareCpu::new();
    let state = cpu.local(Vec::<&'static str>::new());
    let main_state = state.clone();
    let handler_state = state.clone();
    let (main_suspended_tx, main_suspended_rx) = oneshot::channel();
    let (resume_main_tx, resume_main_rx) = oneshot::channel();
    let mut main = Box::pin(cpu.run_main(async move {
        main_state.with(|turns| turns.push("main-before"))?;
        let _ = main_suspended_tx.send(());
        let _ = resume_main_rx.await;
        main_state.with(|turns| turns.push("main-after"))
    }));

    assert_eq!(poll_once(main.as_mut()), Poll::Pending);
    assert_eq!(main_suspended_rx.await, Ok(()));
    let handler = cpu.run_irq(async move { handler_state.with(|turns| turns.push("handler")) });
    assert_eq!(handler.await, Ok(()));
    let _ = resume_main_tx.send(());
    assert_eq!(main.await, Ok(()));

    let turns = cpu
        .run_main(async move { state.with(|turns| turns.clone()) })
        .await;
    assert_eq!(turns, Ok(vec!["main-before", "handler", "main-after"]));
}

#[tokio::test(flavor = "current_thread")]
async fn cancelled_main_releases_context_and_cpu_for_reacquisition() {
    let cpu = SoftwareCpu::new();
    let state = cpu.local(0_u8);
    let cancelled_state = state.clone();
    let (_resume_tx, resume_rx) = oneshot::channel::<()>();
    let mut cancelled = Box::pin(cpu.run_main(async move {
        cancelled_state.with(|value| *value += 1)?;
        let _ = resume_rx.await;
        Ok::<(), ModelError>(())
    }));

    assert_eq!(poll_once(cancelled.as_mut()), Poll::Pending);
    drop(cancelled);
    assert_eq!(
        state.with(|value| *value),
        Err(ModelError::SoftwareCpuInactive)
    );

    let reacquired = cpu
        .run_main(async move { state.with(|value| *value += 1) })
        .await;
    assert_eq!(reacquired, Ok(()));
}

#[tokio::test(flavor = "current_thread")]
async fn handler_identity_is_inactive_between_polls() {
    let cpu = SoftwareCpu::new();
    let state = cpu.local(0_u8);
    let handler_state = state.clone();
    let (resume_tx, resume_rx) = oneshot::channel::<()>();
    let mut handler = Box::pin(cpu.run_irq(async move {
        handler_state.with(|value| *value += 1)?;
        let _ = resume_rx.await;
        handler_state.with(|value| *value += 1)
    }));

    assert_eq!(poll_once(handler.as_mut()), Poll::Pending);
    assert_eq!(
        state.with(|value| *value),
        Err(ModelError::SoftwareCpuInactive)
    );
    let _ = resume_tx.send(());
    assert_eq!(poll_once(handler.as_mut()), Poll::Ready(Ok(())));
}

#[tokio::test(flavor = "current_thread")]
async fn cpu_local_reports_reentrant_state_access_as_busy() {
    let cpu = SoftwareCpu::new();
    let state = cpu.local(0_u8);
    let nested = state.clone();

    let result = cpu
        .run_main(async move { state.with(|_| nested.with(|value| *value)) })
        .await;

    assert_eq!(result, Ok(Err(ModelError::SoftwareCpuStateBusy)));
}
