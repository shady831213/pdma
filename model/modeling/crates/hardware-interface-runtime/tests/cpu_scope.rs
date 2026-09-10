//! Guarding a pending capability before any inner poll on another CPU.

use hardware_interface_runtime::{CpuContextError, CpuOwner, SoftwareCpu};
use std::{cell::Cell, future::poll_fn, task::Poll};

#[tokio::test]
async fn guarded_future_rejects_inactive_cpu_before_side_effects() {
    // Given: a capability captured from a CPU that is no longer active.
    let cpu = SoftwareCpu::new();
    let owner = cpu
        .run_main(async { CpuOwner::capture_active() })
        .await
        .expect("owner");
    let calls = Cell::new(0);
    // When: the guarded operation is driven outside a CPU.
    let result = owner
        .scope(async {
            calls.set(1);
        })
        .await;
    // Then: validation prevents the inner side effect.
    assert_eq!(result, Err(CpuContextError::Inactive));
    assert_eq!(calls.get(), 0);
}

#[tokio::test]
async fn guarded_future_rejects_migration_before_repolling_inner_operation() {
    // Given: a CPU-owned future that suspended after exactly one inner poll.
    let cpu = SoftwareCpu::new();
    let other = SoftwareCpu::new();
    let owner = cpu
        .run_main(async { CpuOwner::capture_active() })
        .await
        .expect("owner");
    let polls = Cell::new(0);
    let guarded = owner.scope(poll_fn(|_| {
        polls.set(polls.get() + 1);
        Poll::Pending::<()>
    }));
    tokio::pin!(guarded);
    cpu.run_main(poll_fn(|cx| {
        assert!(std::future::Future::poll(guarded.as_mut(), cx).is_pending());
        Poll::Ready(())
    }))
    .await;
    // When: a different CPU resumes it.
    let result = other.run_main(guarded).await;
    // Then: the inner operation was never polled on the wrong CPU.
    assert_eq!(result, Err(CpuContextError::Mismatch));
    assert_eq!(polls.get(), 1);
}
