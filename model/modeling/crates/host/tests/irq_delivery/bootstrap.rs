use super::{NotificationPort, message};
use crate::register::legacy_test_support::{IrqDelivery, irq_handler};
use hardware_interface_runtime::SoftwareCpu;
use pdma_model_contract::semantic::{CompletionRingId, IrqSource, IrqVector, QueueId};
use pdma_model_host::interrupt::{InterruptError, InterruptResult};
use tokio::sync::mpsc;

#[tokio::test(flavor = "current_thread")]
async fn batch_registration_publishes_two_handler_types_implicitly() -> InterruptResult<()> {
    // Given: a deferred actor and notifications for two distinct vectors.
    let vector_a = IrqVector::new(31);
    let vector_b = IrqVector::new(32);
    let source_a = IrqSource::Completion(CompletionRingId::new(0));
    let source_b = IrqSource::Event(QueueId::new(1));
    let (port, notifications) = NotificationPort::channel(2);
    notifications
        .send(message(0, vector_a, source_a))
        .await
        .expect("first notification is queued");
    notifications
        .send(message(1, vector_b, source_b))
        .await
        .expect("second notification is queued");
    drop(notifications);
    let (delivery, runner) = IrqDelivery::new(port, SoftwareCpu::new()).defer();
    let (calls_tx, mut calls_rx) = mpsc::unbounded_channel();
    let calls_a = calls_tx.clone();
    let handler_a = irq_handler(move |notification| {
        let calls = calls_a.clone();
        async move {
            let _ = calls.send(("A", notification.vector()));
            Ok(())
        }
    });
    let handler_b = irq_handler(move |notification| {
        let calls = calls_tx.clone();
        async move {
            let _ = calls.send(("B", notification.vector()));
            Ok(())
        }
    });

    // When: software registers both handlers without starting delivery explicitly.
    let _handle = delivery.register_handlers(move |registry| {
        registry.register(vector_a, handler_a)?;
        registry.register(vector_b, handler_b)
    })?;
    let summary = runner.run().await?;

    // Then: registration publishes one fixed registry and dispatches both vectors.
    assert_eq!(summary.deliveries(), 2);
    assert_eq!(calls_rx.recv().await, Some(("A", vector_a)));
    assert_eq!(calls_rx.recv().await, Some(("B", vector_b)));
    Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn empty_batch_registration_does_not_publish_delivery() {
    // Given: a deferred actor with no registered vectors.
    let (port, _notifications) = NotificationPort::channel(1);
    let (delivery, runner) = IrqDelivery::new(port, SoftwareCpu::new()).defer();

    // When: software completes an empty registration transaction.
    let registration = delivery.register_handlers(|_registry| Ok(()));

    // Then: both software and runner observe typed bootstrap failures.
    assert!(matches!(
        registration,
        Err(InterruptError::EmptyRegistry)
    ));
    assert_eq!(runner.run().await, Err(InterruptError::DeliveryUnavailable));
}
use crate::register::legacy_integration_tests::pdma_model_host;
