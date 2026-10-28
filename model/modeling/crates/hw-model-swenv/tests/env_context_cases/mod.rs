use hw_model_swenv::{EnvContextError, EnvLocal, SoftwareEnv};
use std::{future::Future, pin::Pin, task::Poll};

mod local;
mod scheduling;

fn poll_once<F>(future: Pin<&mut F>) -> Poll<F::Output>
where
    F: Future + ?Sized,
{
    let mut context = std::task::Context::from_waker(std::task::Waker::noop());
    future.poll(&mut context)
}

#[test]
fn capture_rejects_inactive_context() {
    assert!(matches!(
        EnvLocal::capture_active(0_u32),
        Err(EnvContextError::Inactive)
    ));
}

#[tokio::test(flavor = "current_thread")]
async fn captured_owner_validates_active_inactive_and_wrong_env() {
    let owner_env = SoftwareEnv::new();
    let wrong_env = SoftwareEnv::new();
    let owner = owner_env
        .run_main(async { EnvLocal::capture_active(0_u32) })
        .await
        .expect("owner environment is active");
    let cloned = owner.clone();

    assert_eq!(owner.with(|value| *value), Err(EnvContextError::Inactive));
    let wrong = wrong_env
        .run_main(async move { cloned.with(|value| *value) })
        .await;
    assert_eq!(wrong, Err(EnvContextError::Mismatch));
    assert_eq!(
        owner_env
            .run_main(async move { owner.with(|value| *value) })
            .await,
        Ok(0)
    );
}
