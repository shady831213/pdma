use super::{EnvContextError, SoftwareEnv, poll_once};
use std::task::Poll;
use tokio::sync::oneshot;

struct NonCloneState {
    value: usize,
}

#[tokio::test(flavor = "current_thread")]
async fn clones_share_one_non_clone_state_and_report_busy_reentry() {
    let env = SoftwareEnv::new();
    let state = env.local(NonCloneState { value: 1 });
    let clone = state.clone();
    let nested = clone.clone();

    let result = env
        .run_main(async move {
            state.with(|state| state.value += 2)?;
            let busy = clone.with(|_| nested.with(|state| state.value));
            Ok::<_, EnvContextError>((busy, clone.with(|state| state.value)?))
        })
        .await;

    assert_eq!(result, Ok((Ok(Err(EnvContextError::StateBusy)), 3)));
}

#[test]
fn local_rejects_inactive_access() {
    let state = SoftwareEnv::new().local(5_u8);
    assert_eq!(state.with(|value| *value), Err(EnvContextError::Inactive));
}

#[tokio::test(flavor = "current_thread")]
async fn nested_different_env_restores_enclosing_owner() {
    let owner = SoftwareEnv::new();
    let other = SoftwareEnv::new();
    let state = owner.local(0_u8);
    let foreign = state.clone();

    let result = owner
        .run_main(async move {
            assert_eq!(
                other
                    .run_main(async move { foreign.with(|value| *value) })
                    .await,
                Err(EnvContextError::Mismatch)
            );
            state.with(|value| *value += 1)
        })
        .await;
    assert_eq!(result, Ok(()));
}

#[tokio::test(flavor = "current_thread")]
async fn handler_token_is_inactive_between_polls_and_after_cancel() {
    let env = SoftwareEnv::new();
    let state = env.local(0_u8);
    let handler_state = state.clone();
    let (_resume_tx, resume_rx) = oneshot::channel::<()>();
    let mut handler = Box::pin(env.run_handler_turn(async move {
        handler_state.with(|value| *value += 1)?;
        let _ = resume_rx.await;
        handler_state.with(|value| *value += 1)
    }));

    assert_eq!(poll_once(handler.as_mut()), Poll::Pending);
    assert_eq!(state.with(|value| *value), Err(EnvContextError::Inactive));
    drop(handler);
    assert_eq!(state.with(|value| *value), Err(EnvContextError::Inactive));
    assert_eq!(
        env.run_main(async move { state.with(|value| *value += 1) })
            .await,
        Ok(())
    );
}
