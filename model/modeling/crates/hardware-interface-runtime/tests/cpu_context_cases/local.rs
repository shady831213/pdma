use super::{CpuContextError, SoftwareCpu, poll_once};
use std::task::Poll;
use tokio::sync::oneshot;

struct NonCloneState {
    value: usize,
}

#[tokio::test(flavor = "current_thread")]
async fn clones_share_one_non_clone_state_and_report_busy_reentry() {
    let cpu = SoftwareCpu::new();
    let state = cpu.local(NonCloneState { value: 1 });
    let clone = state.clone();
    let nested = clone.clone();

    let result = cpu
        .run_main(async move {
            state.with(|state| state.value += 2)?;
            let busy = clone.with(|_| nested.with(|state| state.value));
            Ok::<_, CpuContextError>((busy, clone.with(|state| state.value)?))
        })
        .await;

    assert_eq!(result, Ok((Ok(Err(CpuContextError::StateBusy)), 3)));
}

#[test]
fn local_rejects_inactive_access() {
    let state = SoftwareCpu::new().local(5_u8);
    assert_eq!(state.with(|value| *value), Err(CpuContextError::Inactive));
}

#[tokio::test(flavor = "current_thread")]
async fn nested_different_cpu_restores_enclosing_owner() {
    let owner = SoftwareCpu::new();
    let other = SoftwareCpu::new();
    let state = owner.local(0_u8);
    let foreign = state.clone();

    let result = owner
        .run_main(async move {
            assert_eq!(
                other
                    .run_main(async move { foreign.with(|value| *value) })
                    .await,
                Err(CpuContextError::Mismatch)
            );
            state.with(|value| *value += 1)
        })
        .await;
    assert_eq!(result, Ok(()));
}

#[tokio::test(flavor = "current_thread")]
async fn handler_token_is_inactive_between_polls_and_after_cancel() {
    let cpu = SoftwareCpu::new();
    let state = cpu.local(0_u8);
    let handler_state = state.clone();
    let (_resume_tx, resume_rx) = oneshot::channel::<()>();
    let mut handler = Box::pin(cpu.run_handler_turn(async move {
        handler_state.with(|value| *value += 1)?;
        let _ = resume_rx.await;
        handler_state.with(|value| *value += 1)
    }));

    assert_eq!(poll_once(handler.as_mut()), Poll::Pending);
    assert_eq!(state.with(|value| *value), Err(CpuContextError::Inactive));
    drop(handler);
    assert_eq!(state.with(|value| *value), Err(CpuContextError::Inactive));
    assert_eq!(
        cpu.run_main(async move { state.with(|value| *value += 1) })
            .await,
        Ok(())
    );
}
