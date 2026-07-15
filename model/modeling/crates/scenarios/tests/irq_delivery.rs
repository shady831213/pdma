#![allow(missing_docs)]

include!("irq_delivery/support.rs");

use pdma_model_contract::{
    CopyRequest, IrqNotification, IrqRoute, IrqSource, IrqVector, ModelResult, TraceEvent,
    TraceEventKind,
};
use pdma_model_device::{AsyncDevice, AsyncDeviceHandle};
use pdma_model_host::{
    AsyncDriver, DriverIrqRecord, IrqDelivery, IrqDeliveryHandle, IrqDeliverySummary, IrqHandler,
    IrqHandlerFuture, SoftwareCpu,
};
use tokio::{
    sync::{mpsc, oneshot},
    task::JoinHandle,
};

#[tokio::test(flavor = "current_thread")]
async fn line_completion_runs_device_main_and_delivery_as_concurrent_actors() -> ModelResult<()> {
    // Given: a line-routed completion source and three independently spawned actors.
    let (device, worker) = AsyncDevice::queue_port();
    let worker = tokio::spawn(worker.run());
    let port = device.handle();
    port.enable_tracing().await;
    port.clear_trace().await;
    let cpu = SoftwareCpu::new();
    let (record_tx, mut record_rx) = mpsc::unbounded_channel();
    let (turn_tx, mut turn_rx) = mpsc::unbounded_channel();
    let (driver_tx, driver_rx) = oneshot::channel();
    let delivery = spawn_delivery_after_driver_handoff(
        port.clone(),
        cpu.clone(),
        driver_rx,
        record_tx,
        turn_tx,
        None,
    );

    // When: one complete SW main configures, submits, then suspends for its handler.
    let mut main_port = port.clone();
    let main = tokio::spawn(async move {
        cpu.run_main(async move {
            let mut driver = AsyncDriver::setup_on(&mut main_port, 4, 4).await?;
            let source = IrqSource::Completion(driver.completion_ring_id());
            main_port.enable_irq_source(source).await?;
            main_port.unmask_irq_source(source).await?;
            let src = driver.allocate_buffer_on(&mut main_port, 4).await;
            let dst = driver.allocate_buffer_on(&mut main_port, 4).await;
            driver
                .write_buffer_on(&mut main_port, src, &[4, 3, 2, 1])
                .await?;
            driver
                .submit_copy_on(&mut main_port, CopyRequest::new(src, dst, 4))
                .await?;
            let _ = driver_tx.send(driver);
            let record = record_rx.recv().await;
            let turn = turn_rx.recv().await;
            let copied = main_port.read_buffer(dst, 0, 4).await?;
            let irq_snapshot = main_port.irq_snapshot().await;
            Ok::<_, pdma_model_contract::ModelError>((record, turn, copied, irq_snapshot))
        })
        .await
    });
    let delivery_handle = delivery
        .handle
        .await
        .expect("delivery publishes its host control handle");
    let (record, turn, copied, irq_snapshot) = main.await.expect("SW main task joins cleanly")?;

    // Then: the host handler drains the completion, clears the line, and leaves copied data.
    assert!(matches!(record, Some(DriverIrqRecord::Completion(_))));
    assert_eq!(turn, Some(1));
    assert_eq!(copied, vec![4, 3, 2, 1]);
    assert_eq!(irq_snapshot, None);
    let device_trace = port.trace_events().await;
    let host_trace = delivery_handle.trace_events().await;
    assert_trace_order(
        &device_trace,
        &[
            TraceEventKind::CompletionPublished,
            TraceEventKind::IrqSourceLatched,
            TraceEventKind::IrqLineAsserted,
            TraceEventKind::CompletionPolled,
            TraceEventKind::IrqDeviceAck,
            TraceEventKind::IrqLineDeasserted,
        ],
    );
    assert_trace_order(
        &host_trace,
        &[
            TraceEventKind::IrqHandlerStarted,
            TraceEventKind::IrqHandlerCompleted,
        ],
    );
    delivery_handle.request_shutdown();
    device.request_shutdown().await;
    assert_eq!(
        delivery
            .task
            .await
            .expect("delivery joins cleanly")
            .expect("SW main hands the driver to IRQ delivery")?
            .deliveries(),
        1
    );
    worker.await.expect("device worker joins cleanly");
    Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn message_events_are_demultiplexed_and_drained_by_driver_handler() -> ModelResult<()> {
    // Given: a message-routed event source and three independently spawned actors.
    let (device, worker) = AsyncDevice::queue_port();
    let worker = tokio::spawn(worker.run());
    let port = device.handle();
    port.enable_tracing().await;
    port.clear_trace().await;
    let cpu = SoftwareCpu::new();
    let (record_tx, mut record_rx) = mpsc::unbounded_channel();
    let (turn_tx, mut turn_rx) = mpsc::unbounded_channel();
    let (release_tx, release_rx) = oneshot::channel();
    let (driver_tx, driver_rx) = oneshot::channel();
    let delivery = spawn_delivery_after_driver_handoff(
        port.clone(),
        cpu.clone(),
        driver_rx,
        record_tx,
        turn_tx,
        Some(release_rx),
    );

    // When: one complete SW main submits two faults and waits for both handler turns.
    let mut main_port = port.clone();
    let main = tokio::spawn(async move {
        cpu.run_main(async move {
            let mut driver = AsyncDriver::setup_on(&mut main_port, 4, 4).await?;
            let source = IrqSource::Event(driver.queue_id());
            main_port
                .configure_irq_route(source, IrqRoute::Message(IrqVector::new(13)))
                .await?;
            main_port.enable_irq_source(source).await?;
            main_port.unmask_irq_source(source).await?;
            let src = driver.allocate_buffer_on(&mut main_port, 4).await;
            let dst = driver.allocate_buffer_on(&mut main_port, 4).await;
            for _ in 0..2 {
                driver
                    .submit_copy_on(&mut main_port, CopyRequest::new(src, dst, 8))
                    .await?;
            }
            let _ = driver_tx.send(driver);
            let first_record = record_rx.recv().await;
            let second_record = record_rx.recv().await;
            let first_turn = turn_rx.recv().await;
            let second_turn = turn_rx.recv().await;
            let irq_snapshot = main_port.irq_snapshot().await;
            Ok::<_, pdma_model_contract::ModelError>((
                first_record,
                second_record,
                first_turn,
                second_turn,
                irq_snapshot,
            ))
        })
        .await
    });
    let delivery_handle = delivery
        .handle
        .await
        .expect("delivery publishes its host control handle");
    let error_snapshot = device.wait_for_error_after(1).await;
    assert_eq!(error_snapshot.errors, 2);
    let _ = release_tx.send(());
    let (first_record, second_record, first_turn, second_turn, irq_snapshot) =
        main.await.expect("SW main task joins cleanly")?;

    // Then: one handler drains both events and the redundant message safely drains nothing.
    assert!(matches!(first_record, Some(DriverIrqRecord::Event(_))));
    assert!(matches!(second_record, Some(DriverIrqRecord::Event(_))));
    assert_eq!(first_turn, Some(2));
    assert_eq!(second_turn, Some(0));
    assert_eq!(irq_snapshot, None);
    let device_trace = port.trace_events().await;
    assert_trace_order(
        &device_trace,
        &[
            TraceEventKind::EventPublished,
            TraceEventKind::IrqSourceLatched,
            TraceEventKind::IrqMessageEmitted,
            TraceEventKind::EventPolled,
            TraceEventKind::IrqDeviceAck,
        ],
    );
    assert!(
        !device_trace
            .iter()
            .any(|event| event.kind() == TraceEventKind::IrqLineAsserted)
    );
    delivery_handle.request_shutdown();
    device.request_shutdown().await;
    assert_eq!(
        delivery
            .task
            .await
            .expect("delivery joins cleanly")
            .expect("SW main hands the driver to IRQ delivery")?
            .deliveries(),
        2
    );
    worker.await.expect("device worker joins cleanly");
    Ok(())
}

fn assert_trace_order(events: &[TraceEvent], expected: &[TraceEventKind]) {
    let mut previous = None;
    for kind in expected {
        let position = events
            .iter()
            .position(|event| event.kind() == *kind)
            .unwrap_or(events.len());
        assert!(
            position < events.len(),
            "expected IRQ lifecycle event is traced"
        );
        assert!(previous.is_none_or(|previous| previous < position));
        previous = Some(position);
    }
}
