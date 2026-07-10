#![allow(missing_docs)]

use pdma_model_contract::{
    CopyRequest, IrqNotification, IrqRoute, IrqSource, IrqVector, ModelResult, TraceEvent,
    TraceEventKind,
};
use pdma_model_device::{AsyncDevice, AsyncDeviceHandle};
use pdma_model_host::{
    AsyncDriver, DriverIrqRecord, IrqDelivery, IrqHandler, IrqHandlerFuture, SoftwareCpu,
};
use std::sync::Arc;
use tokio::sync::{Mutex, mpsc};

struct DriverHandler {
    driver: Arc<Mutex<AsyncDriver>>,
    port: AsyncDeviceHandle,
    records: mpsc::UnboundedSender<DriverIrqRecord>,
}

impl IrqHandler for DriverHandler {
    fn handle(&mut self, notification: IrqNotification) -> IrqHandlerFuture<'_> {
        Box::pin(async move {
            let records = self
                .driver
                .lock()
                .await
                .service_irq_on(&mut self.port, notification)
                .await?;
            for record in records {
                let _ = self.records.send(record);
            }
            Ok(())
        })
    }
}

#[tokio::test(flavor = "current_thread")]
async fn line_completion_runs_device_main_and_delivery_as_concurrent_actors() -> ModelResult<()> {
    // Given: a line-routed completion source and three independently spawned actors.
    let (device, worker) = AsyncDevice::queue_port();
    let worker = tokio::spawn(worker.run());
    let mut port = device.handle();
    let driver = Arc::new(Mutex::new(AsyncDriver::setup_on(&mut port, 4, 4).await?));
    let source = IrqSource::Completion(driver.lock().await.completion_ring_id());
    port.enable_irq_source(source).await?;
    port.unmask_irq_source(source).await?;
    port.enable_tracing().await;
    port.clear_trace().await;
    let cpu = SoftwareCpu::new();
    let (record_tx, mut record_rx) = mpsc::unbounded_channel();
    let handler = DriverHandler {
        driver: Arc::clone(&driver),
        port: port.clone(),
        records: record_tx,
    };
    let (delivery, delivery_handle) =
        IrqDelivery::new(port.clone(), cpu.clone()).register_handler(handler);
    let delivery = tokio::spawn(delivery.run());

    // When: SW main submits one successful copy while the other actors keep running.
    let main_driver = Arc::clone(&driver);
    let mut main_port = port.clone();
    let main = tokio::spawn(async move {
        cpu.run_main(async move {
            let mut driver = main_driver.lock().await;
            let src = driver.allocate_buffer_on(&mut main_port, 4).await;
            let dst = driver.allocate_buffer_on(&mut main_port, 4).await;
            driver
                .write_buffer_on(&mut main_port, src, &[4, 3, 2, 1])
                .await?;
            driver
                .submit_copy_on(&mut main_port, CopyRequest::new(src, dst, 4))
                .await?;
            Ok::<_, pdma_model_contract::ModelError>(dst)
        })
        .await
    });
    let dst = main.await.expect("SW main task joins cleanly")?;
    let record = record_rx.recv().await;

    // Then: the host handler drains the completion, clears the line, and leaves copied data.
    assert!(matches!(record, Some(DriverIrqRecord::Completion(_))));
    assert_eq!(port.read_buffer(dst, 0, 4).await?, vec![4, 3, 2, 1]);
    assert_eq!(port.irq_snapshot().await, None);
    delivery_handle.wait_for_deliveries(1).await;
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
            .await
            .expect("delivery joins cleanly")?
            .deliveries(),
        1
    );
    worker.await.expect("device worker joins cleanly");
    Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn message_event_is_demultiplexed_and_drained_by_driver_handler() -> ModelResult<()> {
    // Given: a message-routed event source and three independently spawned actors.
    let (device, worker) = AsyncDevice::queue_port();
    let worker = tokio::spawn(worker.run());
    let mut port = device.handle();
    let driver = Arc::new(Mutex::new(AsyncDriver::setup_on(&mut port, 4, 4).await?));
    let source = IrqSource::Event(driver.lock().await.queue_id());
    port.configure_irq_route(source, IrqRoute::Message(IrqVector::new(13)))
        .await?;
    port.enable_irq_source(source).await?;
    port.unmask_irq_source(source).await?;
    port.enable_tracing().await;
    port.clear_trace().await;
    let cpu = SoftwareCpu::new();
    let (record_tx, mut record_rx) = mpsc::unbounded_channel();
    let handler = DriverHandler {
        driver: Arc::clone(&driver),
        port: port.clone(),
        records: record_tx,
    };
    let (delivery, delivery_handle) =
        IrqDelivery::new(port.clone(), cpu.clone()).register_handler(handler);
    let delivery = tokio::spawn(delivery.run());

    // When: SW main submits a copy whose checked range faults.
    let main_driver = Arc::clone(&driver);
    let mut main_port = port.clone();
    let main = tokio::spawn(async move {
        cpu.run_main(async move {
            let mut driver = main_driver.lock().await;
            let src = driver.allocate_buffer_on(&mut main_port, 4).await;
            let dst = driver.allocate_buffer_on(&mut main_port, 4).await;
            driver
                .submit_copy_on(&mut main_port, CopyRequest::new(src, dst, 8))
                .await
        })
        .await
    });
    main.await.expect("SW main task joins cleanly")?;
    let record = record_rx.recv().await;

    // Then: the same PDMA handler demultiplexes the event and drains it without a completion.
    assert!(matches!(record, Some(DriverIrqRecord::Event(_))));
    assert_eq!(port.irq_snapshot().await, None);
    delivery_handle.wait_for_deliveries(1).await;
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
            .await
            .expect("delivery joins cleanly")?
            .deliveries(),
        1
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
            .expect("expected IRQ lifecycle event is traced");
        assert!(previous.is_none_or(|previous| previous < position));
        previous = Some(position);
    }
}
