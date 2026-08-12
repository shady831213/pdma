use super::{CpuContextError, CpuOwner, SoftwareCpu, poll_once};
use std::{
    future::Future,
    pin::Pin,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    task::{Context, Poll},
};
use tokio::sync::oneshot;

#[tokio::test(flavor = "current_thread")]
async fn real_pending_main_releases_gate_but_ready_chain_does_not() {
    let cpu = SoftwareCpu::new();
    let irq_cpu = cpu.clone();
    let mut ready_main = Box::pin(cpu.run_main(async move {
        let mut handler = Box::pin(irq_cpu.run_handler_turn(async { 7_u8 }));
        assert_eq!(poll_once(handler.as_mut()), Poll::Pending);
        std::future::ready(()).await;
        assert_eq!(poll_once(handler.as_mut()), Poll::Pending);
        11_u8
    }));
    assert_eq!(poll_once(ready_main.as_mut()), Poll::Ready(11));

    let (resume_tx, resume_rx) = oneshot::channel();
    let mut pending_main = Box::pin(cpu.run_main(async move {
        let _ = resume_rx.await;
    }));
    assert_eq!(poll_once(pending_main.as_mut()), Poll::Pending);
    let mut handler = Box::pin(cpu.run_handler_turn(async { 13_u8 }));
    assert_eq!(poll_once(handler.as_mut()), Poll::Ready(13));
    let _ = resume_tx.send(());
    assert_eq!(poll_once(pending_main.as_mut()), Poll::Ready(()));
}

#[tokio::test(flavor = "current_thread")]
async fn pending_handler_blocks_main_and_later_handler_in_fifo_order() {
    let cpu = SoftwareCpu::new();
    let (release_tx, release_rx) = oneshot::channel();
    let mut current = Box::pin(cpu.run_handler_turn(async move {
        let _ = release_rx.await;
    }));
    assert_eq!(poll_once(current.as_mut()), Poll::Pending);
    let mut main = Box::pin(cpu.run_main(async { 17_u8 }));
    assert_eq!(poll_once(main.as_mut()), Poll::Pending);
    let mut later = Box::pin(cpu.run_handler_turn(async { 19_u8 }));
    assert_eq!(poll_once(later.as_mut()), Poll::Pending);

    let _ = release_tx.send(());
    assert_eq!(poll_once(current.as_mut()), Poll::Ready(()));
    assert_eq!(poll_once(later.as_mut()), Poll::Pending);
    assert_eq!(poll_once(main.as_mut()), Poll::Ready(17));
    assert_eq!(poll_once(later.as_mut()), Poll::Ready(19));
}

#[tokio::test(flavor = "current_thread")]
async fn cancelling_pending_main_and_handler_releases_gate() {
    let cpu = SoftwareCpu::new();
    let (_main_tx, main_rx) = oneshot::channel::<()>();
    let mut main = Box::pin(cpu.run_main(async move {
        let _ = main_rx.await;
    }));
    assert_eq!(poll_once(main.as_mut()), Poll::Pending);
    drop(main);
    let (_handler_tx, handler_rx) = oneshot::channel::<()>();
    let mut handler = Box::pin(cpu.run_handler_turn(async move {
        let _ = handler_rx.await;
    }));
    assert_eq!(poll_once(handler.as_mut()), Poll::Pending);
    drop(handler);
    assert_eq!(cpu.run_main(async { 23_u8 }).await, 23);
    assert_eq!(cpu.run_handler_turn(async { 29_u8 }).await, 29);
}

struct DriverLikeFuture {
    owner: CpuOwner,
    commits: Arc<AtomicUsize>,
    pending: bool,
}

impl Future for DriverLikeFuture {
    type Output = Result<(), CpuContextError>;

    fn poll(mut self: Pin<&mut Self>, _context: &mut Context<'_>) -> Poll<Self::Output> {
        if let Err(error) = self.owner.validate_active() {
            return Poll::Ready(Err(error));
        }
        if self.pending {
            self.pending = false;
            return Poll::Pending;
        }
        self.commits.fetch_add(1, Ordering::SeqCst);
        Poll::Ready(Ok(()))
    }
}

#[tokio::test(flavor = "current_thread")]
async fn inactive_and_wrong_cpu_repoll_fail_before_inner_commit() {
    let owner_cpu = SoftwareCpu::new();
    let wrong_cpu = SoftwareCpu::new();
    let owner = owner_cpu
        .run_main(async { CpuOwner::capture_active() })
        .await
        .expect("owner CPU is active");
    let inactive_commits = Arc::new(AtomicUsize::new(0));
    let mut inactive_operation = Box::pin(DriverLikeFuture {
        owner: owner.clone(),
        commits: Arc::clone(&inactive_commits),
        pending: true,
    });
    owner_cpu
        .run_main(async { assert_eq!(poll_once(inactive_operation.as_mut()), Poll::Pending) })
        .await;
    assert_eq!(
        poll_once(inactive_operation.as_mut()),
        Poll::Ready(Err(CpuContextError::Inactive))
    );
    assert_eq!(inactive_commits.load(Ordering::SeqCst), 0);

    let wrong_commits = Arc::new(AtomicUsize::new(0));
    let mut operation = Box::pin(DriverLikeFuture {
        owner,
        commits: Arc::clone(&wrong_commits),
        pending: true,
    });
    owner_cpu
        .run_main(async { assert_eq!(poll_once(operation.as_mut()), Poll::Pending) })
        .await;
    let result = wrong_cpu.run_main(operation.as_mut()).await;
    assert_eq!(result, Err(CpuContextError::Mismatch));
    assert_eq!(wrong_commits.load(Ordering::SeqCst), 0);
}
