use hardware_interface_runtime::{CpuContextError, CpuOwner, SoftwareCpu};
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
        CpuOwner::capture_active(),
        Err(CpuContextError::Inactive)
    ));
}

#[tokio::test(flavor = "current_thread")]
async fn captured_owner_validates_active_inactive_and_wrong_cpu() {
    let owner_cpu = SoftwareCpu::new();
    let wrong_cpu = SoftwareCpu::new();
    let owner = owner_cpu
        .run_main(async { CpuOwner::capture_active() })
        .await
        .expect("owner CPU is active");
    let cloned = owner.clone();

    assert_eq!(owner.validate_active(), Err(CpuContextError::Inactive));
    let wrong = wrong_cpu
        .run_main(async move { cloned.validate_active() })
        .await;
    assert_eq!(wrong, Err(CpuContextError::Mismatch));
    assert_eq!(
        owner_cpu
            .run_main(async move { owner.validate_active() })
            .await,
        Ok(())
    );
}
