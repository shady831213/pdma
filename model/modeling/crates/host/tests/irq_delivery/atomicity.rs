use crate::{NotificationPort, message};
use pdma_model_contract::{
    CopyRequest, IrqNotification, IrqRoute, IrqSource, IrqVector, ModelResult,
};
use pdma_model_device::AsyncDevice;
use pdma_model_host::{AsyncDriver, IrqDelivery, IrqHandler, IrqHandlerFuture, SoftwareCpu};
use tokio::sync::{mpsc, oneshot};

struct BlockingHandler {
    calls: usize,
    started: mpsc::UnboundedSender<usize>,
    completed: mpsc::UnboundedSender<usize>,
    release_first: Option<oneshot::Receiver<()>>,
}

impl IrqHandler for BlockingHandler {
    fn handle(&mut self, _notification: IrqNotification) -> IrqHandlerFuture<'_> {
        Box::pin(async move {
            self.calls = self.calls.saturating_add(1);
            let _ = self.started.send(self.calls);
            if let Some(release) = self.release_first.take() {
                let _ = release.await;
            }
            let _ = self.completed.send(self.calls);
            Ok(())
        })
    }
}

struct SequencedHandler {
    started: &'static str,
    completed: &'static str,
    order: mpsc::UnboundedSender<&'static str>,
    release: Option<oneshot::Receiver<()>>,
}

impl IrqHandler for SequencedHandler {
    fn handle(&mut self, _notification: IrqNotification) -> IrqHandlerFuture<'_> {
        Box::pin(async move {
            let _ = self.order.send(self.started);
            if let Some(release) = self.release.take() {
                let _ = release.await;
            }
            let _ = self.order.send(self.completed);
            Ok(())
        })
    }
}

#[tokio::test(flavor = "current_thread")]
async fn handler_is_atomic_against_main_and_irq_nesting() -> ModelResult<()> {
    // Given: two queued IRQ messages, one CPU token, and a handler blocked on its first call.
    let (device, worker) = AsyncDevice::queue_port();
    let worker = tokio::spawn(worker.run());
    let mut device_port = device.handle();
    let cpu = SoftwareCpu::new();
    let (mut driver, src, first_dst, second_dst) = cpu
        .run_main(async {
            let driver = AsyncDriver::setup_on(&mut device_port, 4, 4).await?;
            let source = IrqSource::Completion(driver.completion_ring_id());
            device_port
                .configure_irq_route(source, IrqRoute::Message(IrqVector::new(7)))
                .await?;
            device_port.enable_irq_source(source).await?;
            device_port.unmask_irq_source(source).await?;
            let src = driver.allocate_buffer_on(&mut device_port, 4).await;
            let first_dst = driver.allocate_buffer_on(&mut device_port, 4).await;
            let second_dst = driver.allocate_buffer_on(&mut device_port, 4).await;
            driver
                .write_buffer_on(&mut device_port, src, &[1, 2, 3, 4])
                .await?;
            Ok::<_, pdma_model_contract::ModelError>((driver, src, first_dst, second_dst))
        })
        .await?;
    let (started_tx, mut started_rx) = mpsc::unbounded_channel();
    let (completed_tx, mut completed_rx) = mpsc::unbounded_channel();
    let (release_tx, release_rx) = oneshot::channel();
    let handler = BlockingHandler {
        calls: 0,
        started: started_tx,
        completed: completed_tx,
        release_first: Some(release_rx),
    };
    let (delivery, delivery_handle) = IrqDelivery::new(device_port.clone(), cpu.clone())
        .register_handler(IrqVector::new(7), handler);
    let delivery = tokio::spawn(delivery.run());

    cpu.run_main(async {
        driver
            .submit_copy_on(&mut device_port, CopyRequest::new(src, first_dst, 4))
            .await?;
        driver
            .submit_copy_on(&mut device_port, CopyRequest::new(src, second_dst, 4))
            .await?;
        Ok::<_, pdma_model_contract::ModelError>(())
    })
    .await?;
    assert_eq!(started_rx.recv().await, Some(1));

    // When: SW main requests the CPU while the first handler still owns it.
    let (main_tx, mut main_rx) = mpsc::unbounded_channel();
    let main_cpu = cpu.clone();
    let main = tokio::spawn(async move {
        main_cpu
            .run_main(async move {
                let _ = main_tx.send(());
            })
            .await;
    });
    tokio::task::yield_now().await;

    // Then: neither SW main nor a nested handler can run before the first handler returns.
    assert!(main_rx.try_recv().is_err());
    assert!(started_rx.try_recv().is_err());
    let _ = release_tx.send(());
    assert_eq!(main_rx.recv().await, Some(()));
    assert_eq!(started_rx.recv().await, Some(2));
    assert_eq!(completed_rx.recv().await, Some(1));
    assert_eq!(completed_rx.recv().await, Some(2));
    delivery_handle.request_shutdown();
    device.request_shutdown().await;
    main.await.expect("SW main task joins cleanly");
    let summary = delivery.await.expect("delivery task joins cleanly")?;
    worker.await.expect("device worker joins cleanly");
    assert_eq!(summary.deliveries(), 2);
    Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn pending_handler_blocks_main_and_another_vector() -> ModelResult<()> {
    // Given: vector A owns the CPU, main is queued, and vector B is pending behind A.
    let cpu = SoftwareCpu::new();
    let vector_a = IrqVector::new(31);
    let vector_b = IrqVector::new(32);
    let source = IrqSource::Completion(pdma_model_contract::CompletionRingId::new(0));
    let (port, notifications) = NotificationPort::channel(2);
    notifications
        .send(message(0, vector_a, source))
        .await
        .expect("vector A notification is queued");
    notifications
        .send(message(1, vector_b, source))
        .await
        .expect("vector B notification is queued");
    drop(notifications);
    let (order_tx, mut order_rx) = mpsc::unbounded_channel();
    let (release_tx, release_rx) = oneshot::channel();
    let (mut delivery, _handle) = IrqDelivery::new(port, cpu.clone()).register_handler(
        vector_a,
        SequencedHandler {
            started: "handler-a-started",
            completed: "handler-a-completed",
            order: order_tx.clone(),
            release: Some(release_rx),
        },
    );
    delivery.register_handler(
        vector_b,
        SequencedHandler {
            started: "handler-b-started",
            completed: "handler-b-completed",
            order: order_tx.clone(),
            release: None,
        },
    )?;
    let delivery = tokio::spawn(delivery.run());
    assert_eq!(order_rx.recv().await, Some("handler-a-started"));
    let main_cpu = cpu.clone();
    let main = tokio::spawn(async move {
        main_cpu
            .run_main(async move {
                let _ = order_tx.send("main");
            })
            .await;
    });
    tokio::task::yield_now().await;

    // When: handler A returns and releases the shared FIFO CPU gate.
    assert!(order_rx.try_recv().is_err());
    let _ = release_tx.send(());

    // Then: main resumes before vector B and no handler execution overlaps or nests.
    assert_eq!(order_rx.recv().await, Some("handler-a-completed"));
    assert_eq!(order_rx.recv().await, Some("main"));
    assert_eq!(order_rx.recv().await, Some("handler-b-started"));
    assert_eq!(order_rx.recv().await, Some("handler-b-completed"));
    main.await.expect("SW main task joins cleanly");
    let summary = delivery.await.expect("delivery task joins cleanly")?;
    assert_eq!(summary.deliveries(), 2);
    Ok(())
}
