//! Coalesced wakeups without interrupt dispatch.

use hardware_interface_software::{NotificationWait, notifications};
use std::{
    future::Future,
    pin::pin,
    task::{Context, Waker},
};

#[tokio::test]
async fn notification_before_wait_is_retained_and_coalesces() {
    // Given: no receiver is currently waiting.
    let (sender, receiver) = notifications();
    // When: the producer sends repeated progress hints.
    sender.notify();
    sender.notify();
    // Then: one wait succeeds, and the next one remains pending.
    receiver.wait_notification().await;
    let mut wait = pin!(receiver.wait_notification());
    assert!(
        wait.as_mut()
            .poll(&mut Context::from_waker(Waker::noop()))
            .is_pending()
    );
}

#[tokio::test]
async fn cancelling_an_awakened_wait_preserves_the_notification() {
    // Given: a registered waiter and a notification arriving before it repolls.
    let (sender, receiver) = notifications();
    {
        let mut wait = pin!(receiver.wait_notification());
        assert!(
            wait.as_mut()
                .poll(&mut Context::from_waker(Waker::noop()))
                .is_pending()
        );
        sender.notify();
        // When: the registered future is dropped.
    }
    // Then: a new waiter can consume the retained progress hint.
    let mut wait = pin!(receiver.wait_notification());
    assert!(
        wait.as_mut()
            .poll(&mut Context::from_waker(Waker::noop()))
            .is_ready()
    );
}

#[tokio::test]
async fn notification_destinations_are_independent() {
    // Given: independent firmware and Host destinations.
    let (to_firmware, firmware) = notifications();
    let (_to_host, host) = notifications();
    // When: only firmware is notified.
    to_firmware.notify();
    // Then: Host remains parked, while firmware can proceed.
    let mut waiting = pin!(host.wait_notification());
    assert!(
        waiting
            .as_mut()
            .poll(&mut Context::from_waker(Waker::noop()))
            .is_pending()
    );
    firmware.wait_notification().await;
}
#[tokio::test]
async fn receiver_clones_share_one_consumable_hint() {
    // Given: two handles to the same notification destination.
    let (sender, receiver) = notifications();
    let other = receiver.clone();
    // When: one handle consumes the single emitted hint.
    sender.notify();
    other.wait_notification().await;
    // Then: the original handle cannot consume that hint a second time.
    let mut wait = pin!(receiver.wait_notification());
    assert!(
        wait.as_mut()
            .poll(&mut Context::from_waker(Waker::noop()))
            .is_pending()
    );
}
