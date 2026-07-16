use super::SoftwareCpu;
use std::{future::Future, pin::Pin, task::Poll};
use tokio::sync::oneshot;

mod cpu_local;

fn poll_once<F>(future: Pin<&mut F>) -> Poll<F::Output>
where
    F: Future + ?Sized,
{
    let mut context = std::task::Context::from_waker(std::task::Waker::noop());
    future.poll(&mut context)
}

#[tokio::test(flavor = "current_thread")]
async fn suspended_main_admits_irq_between_pending_segments() {
    // Given: a complete main program suspended after entering its first segment.
    let cpu = SoftwareCpu::new();
    let main_cpu = cpu.clone();
    let (entered_tx, entered_rx) = oneshot::channel();
    let (resume_tx, resume_rx) = oneshot::channel();
    let main = tokio::spawn(async move {
        main_cpu
            .run_main(async move {
                let _ = entered_tx.send(());
                let _ = resume_rx.await;
            })
            .await;
    });
    assert!(
        entered_rx.await.is_ok(),
        "main enters its first CPU segment"
    );

    // When: one IRQ turn is polled while main is suspended at a real await boundary.
    let mut irq = Box::pin(cpu.run_irq(async { 7_u8 }));
    let result = poll_once(irq.as_mut());

    // Then: main released the CPU, so the complete IRQ turn finishes immediately.
    assert_eq!(result, Poll::Ready(7));
    let _ = resume_tx.send(());
    assert!(main.await.is_ok(), "main task joins cleanly");
}

#[tokio::test(flavor = "current_thread")]
async fn pending_handler_blocks_ready_main() {
    // Given: one IRQ handler suspended while owning the modeled CPU.
    let cpu = SoftwareCpu::new();
    let (release_tx, release_rx) = oneshot::channel();
    let mut irq = Box::pin(cpu.run_irq(async move {
        let _ = release_rx.await;
    }));
    assert_eq!(poll_once(irq.as_mut()), Poll::Pending);

    // When: a ready main program requests the same CPU.
    let mut main = Box::pin(cpu.run_main(async { 11_u8 }));
    let blocked = poll_once(main.as_mut());

    // Then: main stays queued until the whole handler future completes.
    assert_eq!(blocked, Poll::Pending);
    let _ = release_tx.send(());
    assert_eq!(poll_once(irq.as_mut()), Poll::Ready(()));
    assert_eq!(poll_once(main.as_mut()), Poll::Ready(11));
}

#[tokio::test(flavor = "current_thread")]
async fn ready_await_chain_remains_one_cpu_segment() {
    // Given: a main program with only immediately ready await points.
    let cpu = SoftwareCpu::new();
    let irq_cpu = cpu.clone();
    let mut main = Box::pin(cpu.run_main(async move {
        let mut irq = Box::pin(irq_cpu.run_irq(async { 13_u8 }));
        assert_eq!(poll_once(irq.as_mut()), Poll::Pending);
        std::future::ready(()).await;
        assert_eq!(poll_once(irq.as_mut()), Poll::Pending);
        17_u8
    }));

    // When: the complete main program is polled once.
    let result = poll_once(main.as_mut());

    // Then: ready awaits do not release the CPU to the queued IRQ.
    assert_eq!(result, Poll::Ready(17));
}

#[tokio::test(flavor = "current_thread")]
async fn nested_irq_waits_for_current_handler() {
    // Given: one IRQ handler suspended while holding the CPU.
    let cpu = SoftwareCpu::new();
    let (release_tx, release_rx) = oneshot::channel();
    let mut first = Box::pin(cpu.run_irq(async move {
        let _ = release_rx.await;
    }));
    assert_eq!(poll_once(first.as_mut()), Poll::Pending);

    // When: a second IRQ handler requests the CPU.
    let mut second = Box::pin(cpu.run_irq(async { 19_u8 }));
    let nested = poll_once(second.as_mut());

    // Then: the second handler cannot nest inside the first.
    assert_eq!(nested, Poll::Pending);
    let _ = release_tx.send(());
    assert_eq!(poll_once(first.as_mut()), Poll::Ready(()));
    assert_eq!(poll_once(second.as_mut()), Poll::Ready(19));
}

#[tokio::test(flavor = "current_thread")]
async fn cancelled_handler_releases_cpu() {
    // Given: one suspended IRQ handler that owns the CPU.
    let cpu = SoftwareCpu::new();
    let (_release_tx, release_rx) = oneshot::channel::<()>();
    let mut irq = Box::pin(cpu.run_irq(async move {
        let _ = release_rx.await;
    }));
    assert_eq!(poll_once(irq.as_mut()), Poll::Pending);

    // When: the handler future is cancelled by being dropped.
    drop(irq);
    let mut main = Box::pin(cpu.run_main(async { 23_u8 }));

    // Then: mutex RAII releases the CPU for the next software context.
    assert_eq!(poll_once(main.as_mut()), Poll::Ready(23));
}

#[tokio::test(flavor = "current_thread")]
async fn queued_main_is_not_overtaken_by_later_irq() {
    // Given: a handler owns the CPU, with main queued before another IRQ.
    let cpu = SoftwareCpu::new();
    let (release_tx, release_rx) = oneshot::channel();
    let mut current_irq = Box::pin(cpu.run_irq(async move {
        let _ = release_rx.await;
    }));
    assert_eq!(poll_once(current_irq.as_mut()), Poll::Pending);
    let mut main = Box::pin(cpu.run_main(async { 29_u8 }));
    assert_eq!(poll_once(main.as_mut()), Poll::Pending);
    let mut later_irq = Box::pin(cpu.run_irq(async { 31_u8 }));
    assert_eq!(poll_once(later_irq.as_mut()), Poll::Pending);

    // When: the current handler releases the CPU.
    let _ = release_tx.send(());
    assert_eq!(poll_once(current_irq.as_mut()), Poll::Ready(()));

    // Then: the persisted main acquisition keeps its FIFO position.
    assert_eq!(poll_once(later_irq.as_mut()), Poll::Pending);
    assert_eq!(poll_once(main.as_mut()), Poll::Ready(29));
    assert_eq!(poll_once(later_irq.as_mut()), Poll::Ready(31));
}
