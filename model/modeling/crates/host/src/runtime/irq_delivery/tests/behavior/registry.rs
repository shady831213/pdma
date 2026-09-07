use super::{NotificationPort, line, message};
use crate::runtime::interrupt::{InterruptError, InterruptResult};
use crate::runtime::irq_delivery::{IrqDelivery, IrqHandler, IrqHandlerFuture};
use hardware_interface_runtime::SoftwareCpu;
use pdma_model_contract::semantic::{
    CompletionRingId, IrqNotification, IrqSource, IrqVector, QueueId, TraceEventKind,
};
use std::sync::atomic::{AtomicUsize, Ordering};
use tokio::sync::{mpsc, oneshot};

struct RecordingHandler<'a> {
    identity: &'static str,
    borrowed_calls: &'a AtomicUsize,
    calls: mpsc::UnboundedSender<(&'static str, usize, IrqSource)>,
}

impl IrqHandler for RecordingHandler<'_> {
    fn handle(&mut self, notification: IrqNotification) -> IrqHandlerFuture<'_> {
        Box::pin(async move {
            let call = self.borrowed_calls.fetch_add(1, Ordering::SeqCst) + 1;
            let source = match notification {
                IrqNotification::Line(snapshot) => snapshot.cause().source(),
                IrqNotification::Message(message) => message.source(),
            };
            let _ = self.calls.send((self.identity, call, source));
            Ok(())
        })
    }
}

struct HoldingHandler {
    entered: Option<oneshot::Sender<()>>,
    release: Option<oneshot::Receiver<()>>,
}

impl IrqHandler for HoldingHandler {
    fn handle(&mut self, _notification: IrqNotification) -> IrqHandlerFuture<'_> {
        Box::pin(async move {
            if let Some(entered) = self.entered.take() {
                let _ = entered.send(());
            }
            if let Some(release) = self.release.take() {
                let _ = release.await;
            }
            Ok(())
        })
    }
}

#[tokio::test(flavor = "current_thread")]
async fn different_vectors_dispatch_to_distinct_handlers() -> InterruptResult<()> {
    // Given: borrowed, heterogeneous handler instances registered for two vectors.
    let vector_a = IrqVector::new(21);
    let vector_b = IrqVector::new(22);
    let source_a = IrqSource::Completion(CompletionRingId::new(0));
    let source_b = IrqSource::Event(QueueId::new(1));
    let calls_a = AtomicUsize::new(0);
    let calls_b = AtomicUsize::new(0);
    let (calls_tx, mut calls_rx) = mpsc::unbounded_channel();
    let (port, notifications) = NotificationPort::channel(2);
    notifications
        .send(line(vector_a, source_a))
        .await
        .expect("line notification is queued");
    notifications
        .send(message(0, vector_b, source_b))
        .await
        .expect("message notification is queued");
    drop(notifications);
    let (mut delivery, handle) = IrqDelivery::new(port, SoftwareCpu::new()).register_handler(
        vector_a,
        RecordingHandler {
            identity: "A",
            borrowed_calls: &calls_a,
            calls: calls_tx.clone(),
        },
    );
    delivery.register_handler(
        vector_b,
        RecordingHandler {
            identity: "B",
            borrowed_calls: &calls_b,
            calls: calls_tx,
        },
    )?;
    let _debug = format!("{delivery:?}");

    // When: a Line and Message notification are delivered in source order.
    let summary = delivery.run().await?;

    // Then: each vector selects its own persistent handler.
    assert_eq!(summary.deliveries(), 2);
    assert_eq!(calls_rx.recv().await, Some(("A", 1, source_a)));
    assert_eq!(calls_rx.recv().await, Some(("B", 1, source_b)));
    assert_eq!(calls_a.load(Ordering::SeqCst), 1);
    assert_eq!(calls_b.load(Ordering::SeqCst), 1);
    let trace = handle
        .trace_events()
        .await
        .into_iter()
        .map(|event| (event.kind(), event.detail()))
        .collect::<Vec<_>>();
    assert_eq!(
        trace,
        [
            (TraceEventKind::IrqHandlerStarted, "line"),
            (TraceEventKind::IrqHandlerCompleted, "Success"),
            (TraceEventKind::IrqHandlerStarted, "message"),
            (TraceEventKind::IrqHandlerCompleted, "Success"),
        ]
    );
    Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn sources_sharing_a_vector_use_one_persistent_handler() -> InterruptResult<()> {
    // Given: two sources routed to one registered vector.
    let vector = IrqVector::new(23);
    let source_a = IrqSource::Completion(CompletionRingId::new(0));
    let source_b = IrqSource::Event(QueueId::new(1));
    let calls = AtomicUsize::new(0);
    let (calls_tx, mut calls_rx) = mpsc::unbounded_channel();
    let (port, notifications) = NotificationPort::channel(2);
    notifications
        .send(message(0, vector, source_a))
        .await
        .expect("first source notification is queued");
    notifications
        .send(message(0, vector, source_b))
        .await
        .expect("second source notification is queued");
    drop(notifications);
    let (delivery, _handle) = IrqDelivery::new(port, SoftwareCpu::new()).register_handler(
        vector,
        RecordingHandler {
            identity: "shared",
            borrowed_calls: &calls,
            calls: calls_tx,
        },
    );

    // When: both messages are delivered.
    let summary = delivery.run().await?;

    // Then: source demultiplexing observes persistent per-handler call state.
    assert_eq!(summary.deliveries(), 2);
    assert_eq!(calls_rx.recv().await, Some(("shared", 1, source_a)));
    assert_eq!(calls_rx.recv().await, Some(("shared", 2, source_b)));
    Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn duplicate_vector_keeps_the_original_handler() -> InterruptResult<()> {
    // Given: one registered vector and a distinct replacement candidate.
    let vector = IrqVector::new(24);
    let source = IrqSource::Completion(CompletionRingId::new(0));
    let original_calls = AtomicUsize::new(0);
    let replacement_calls = AtomicUsize::new(0);
    let (calls_tx, mut calls_rx) = mpsc::unbounded_channel();
    let (port, notifications) = NotificationPort::channel(1);
    notifications
        .send(message(0, vector, source))
        .await
        .expect("notification is queued");
    drop(notifications);
    let (mut delivery, _handle) = IrqDelivery::new(port, SoftwareCpu::new()).register_handler(
        vector,
        RecordingHandler {
            identity: "original",
            borrowed_calls: &original_calls,
            calls: calls_tx.clone(),
        },
    );

    // When: registration attempts to replace the same vector.
    let result = delivery.register_handler(
        vector,
        RecordingHandler {
            identity: "replacement",
            borrowed_calls: &replacement_calls,
            calls: calls_tx,
        },
    );

    // Then: the typed error leaves the original mapping intact.
    assert_eq!(result, Err(InterruptError::DuplicateVector(vector)));
    assert_eq!(delivery.run().await?.deliveries(), 1);
    assert_eq!(calls_rx.recv().await, Some(("original", 1, source)));
    assert_eq!(replacement_calls.load(Ordering::SeqCst), 0);
    Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn unregistered_vector_fails_before_cpu_and_trace() -> InterruptResult<()> {
    // Given: one handler holds the CPU while another actor has an unregistered vector ready.
    let registered = IrqVector::new(25);
    let unregistered = IrqVector::new(26);
    let source = IrqSource::Completion(CompletionRingId::new(0));
    let calls = AtomicUsize::new(0);
    let (calls_tx, _calls_rx) = mpsc::unbounded_channel();
    let (port, notifications) = NotificationPort::channel(1);
    notifications
        .send(message(0, unregistered, source))
        .await
        .expect("unregistered notification is queued");
    drop(notifications);
    let cpu = SoftwareCpu::new();
    let (holder_port, holder_notifications) = NotificationPort::channel(1);
    holder_notifications
        .send(message(0, registered, source))
        .await
        .expect("holder notification is queued");
    drop(holder_notifications);
    let (entered_tx, entered_rx) = oneshot::channel();
    let (release_tx, release_rx) = oneshot::channel();
    let (holder, _holder_handle) = IrqDelivery::new(holder_port, cpu.clone()).register_handler(
        registered,
        HoldingHandler {
            entered: Some(entered_tx),
            release: Some(release_rx),
        },
    );
    let holder = tokio::spawn(holder.run());
    let _ = entered_rx.await;
    let (delivery, handle) = IrqDelivery::new(port, cpu.clone()).register_handler(
        registered,
        RecordingHandler {
            identity: "registered",
            borrowed_calls: &calls,
            calls: calls_tx,
        },
    );

    // When: delivery resolves the unknown vector while CPU acquisition would block.
    let result = delivery.run().await;

    // Then: lookup fails immediately and emits no handler lifecycle trace.
    assert_eq!(
        result,
        Err(InterruptError::UnregisteredVector(unregistered))
    );
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    assert!(handle.trace_events().await.is_empty());
    let _ = release_tx.send(());
    let summary = holder.await.expect("CPU holder joins cleanly")?;
    assert_eq!(summary.deliveries(), 1);
    Ok(())
}
