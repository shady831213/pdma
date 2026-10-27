//! Behavioral boundaries for a coalesced pending interrupt destination.

use hardware_interface_irq::{InterruptEnvelope, InterruptSource};
use std::{
    future::Future,
    pin::pin,
    task::{Context, Poll, Waker},
};

#[derive(Clone, Debug, PartialEq, Eq)]
struct Notice(u8);

impl InterruptEnvelope for Notice {
    type Vector = u8;

    fn vector(&self) -> u8 {
        self.0
    }
}

fn poll_once<F: Future>(future: F) -> Poll<F::Output> {
    pin!(future).poll(&mut Context::from_waker(Waker::noop()))
}

#[tokio::test]
async fn notification_raised_before_wait_is_retained() {
    // Given: a fresh destination with no registered waiter.
    let (emitter, mut source) = hardware_interface_irq::interrupt_wire(Notice(3));
    assert!(!source.is_pending());
    // When: hardware raises before software waits.
    emitter.raise();
    // Then: readiness is immediate and claims the configured envelope.
    assert!(source.is_pending());
    assert!(matches!(
        poll_once(source.wait_ready()),
        Poll::Ready(Ok(()))
    ));
    assert_eq!(source.claim(()).await.expect("claim"), Some(Notice(3)));
}

#[tokio::test]
async fn repeated_raises_coalesce_until_claim() {
    // Given: multiple emitters for one destination.
    let (emitter, mut source) = hardware_interface_irq::interrupt_wire(Notice(3));
    let other = emitter.clone();
    // When: both raise before a claim.
    emitter.raise();
    other.raise();
    emitter.raise();
    // Then: only one notification can be claimed.
    assert_eq!(source.claim(()).await.expect("claim"), Some(Notice(3)));
    assert_eq!(source.claim(()).await.expect("claim"), None);
    assert!(!source.is_pending());
    assert!(poll_once(source.wait_ready()).is_pending());
}

#[tokio::test]
async fn canceling_a_wait_preserves_a_later_raise() {
    // Given: a suspended wait which is dropped after its initial poll.
    let (emitter, mut source) = hardware_interface_irq::interrupt_wire(Notice(3));
    assert!(poll_once(source.wait_ready()).is_pending());
    // When: a notification arrives after cancellation.
    emitter.raise();
    // Then: a replacement wait observes it without consuming it.
    assert!(matches!(
        poll_once(source.wait_ready()),
        Poll::Ready(Ok(()))
    ));
    assert!(matches!(
        poll_once(source.wait_ready()),
        Poll::Ready(Ok(()))
    ));
    assert_eq!(source.claim(()).await.expect("claim"), Some(Notice(3)));
}

#[tokio::test]
async fn canceled_awakened_wait_does_not_consume_notification() {
    // Given: a real watch wait registered while the source is idle.
    let (emitter, mut source) = hardware_interface_irq::interrupt_wire(Notice(3));
    let mut wait = Box::pin(source.wait_ready());
    assert!(
        wait.as_mut()
            .poll(&mut Context::from_waker(Waker::noop()))
            .is_pending()
    );
    // When: hardware raises and the awakened wait is canceled before repolling.
    emitter.raise();
    drop(wait);
    // Then: another wait and claim retain the notification.
    assert!(matches!(
        poll_once(source.wait_ready()),
        Poll::Ready(Ok(()))
    ));
    assert_eq!(source.claim(()).await.expect("claim"), Some(Notice(3)));
}

#[tokio::test]
async fn cloned_ready_sources_compete_for_one_claim() {
    // Given: two source handles observing the same destination.
    let (emitter, mut first) = hardware_interface_irq::interrupt_wire(Notice(3));
    let mut second = first.clone();
    emitter.raise();
    assert!(matches!(poll_once(first.wait_ready()), Poll::Ready(Ok(()))));
    assert!(matches!(
        poll_once(second.wait_ready()),
        Poll::Ready(Ok(()))
    ));
    // When: both attempt to claim their readiness.
    let claims = tokio::join!(first.claim(()), second.claim(()));
    // Then: readiness did not reserve two copies of the notification.
    let claims = [
        claims.0.expect("first claim"),
        claims.1.expect("second claim"),
    ];
    assert_eq!(
        claims
            .iter()
            .filter(|claim| **claim == Some(Notice(3)))
            .count(),
        1
    );
    assert_eq!(claims.iter().filter(|claim| claim.is_none()).count(), 1);
}

#[tokio::test]
async fn raise_after_claim_creates_a_new_pending_notification() {
    // Given: an earlier notification was already claimed.
    let (emitter, mut source) = hardware_interface_irq::interrupt_wire(Notice(3));
    emitter.raise();
    assert_eq!(source.claim(()).await.expect("claim"), Some(Notice(3)));
    // When: hardware raises again after the claim boundary.
    emitter.raise();
    // Then: a new notification survives independently of the previous claim.
    assert!(matches!(
        poll_once(source.wait_ready()),
        Poll::Ready(Ok(()))
    ));
    assert_eq!(source.claim(()).await.expect("claim"), Some(Notice(3)));
    assert_eq!(source.claim(()).await.expect("claim"), None);
}

#[tokio::test]
async fn destinations_keep_independent_pending_state_and_vectors() {
    // Given: independent destinations with distinct fixed vectors.
    let (first_emitter, mut first) = hardware_interface_irq::interrupt_wire(Notice(3));
    let (second_emitter, mut second) = hardware_interface_irq::interrupt_wire(Notice(7));
    first_emitter.raise();
    // When: the second destination is also raised.
    second_emitter.raise();
    // Then: claiming one cannot retract the other's notification or vector.
    assert_eq!(first.claim(()).await.expect("claim"), Some(Notice(3)));
    assert!(matches!(
        poll_once(second.wait_ready()),
        Poll::Ready(Ok(()))
    ));
    assert_eq!(second.claim(()).await.expect("claim"), Some(Notice(7)));
    assert_eq!(first.claim(()).await.expect("claim"), None);
}

#[test]
fn dropping_emitters_does_not_close_a_live_source() {
    // Given: one idle destination.
    let (emitter, source) = hardware_interface_irq::interrupt_wire(Notice(3));
    // When: the last emission capability is dropped.
    drop(emitter);
    // Then: the source keeps waiting; emitter lifetime is not end-of-stream.
    assert!(poll_once(source.wait_ready()).is_pending());
}
