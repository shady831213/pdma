struct DeliveryActor {
    handle: oneshot::Receiver<IrqDeliveryHandle>,
    task: JoinHandle<Result<ModelResult<IrqDeliverySummary>, oneshot::error::RecvError>>,
}

struct DriverHandler {
    driver: AsyncDriver,
    port: AsyncDeviceHandle,
    records: mpsc::UnboundedSender<DriverIrqRecord>,
    turns: mpsc::UnboundedSender<usize>,
    release_first: Option<oneshot::Receiver<()>>,
}

impl IrqHandler for DriverHandler {
    fn handle(&mut self, notification: IrqNotification) -> IrqHandlerFuture<'_> {
        Box::pin(async move {
            if let Some(release) = self.release_first.take() {
                let _ = release.await;
            }
            let records = self
                .driver
                .service_irq_on(&mut self.port, notification)
                .await?;
            let _ = self.turns.send(records.len());
            for record in records {
                let _ = self.records.send(record);
            }
            Ok(())
        })
    }
}

fn spawn_delivery_after_driver_handoff(
    port: AsyncDeviceHandle,
    cpu: SoftwareCpu,
    driver: oneshot::Receiver<AsyncDriver>,
    records: mpsc::UnboundedSender<DriverIrqRecord>,
    turns: mpsc::UnboundedSender<usize>,
    release_first: Option<oneshot::Receiver<()>>,
) -> DeliveryActor {
    let (handle_tx, handle) = oneshot::channel();
    let task = tokio::spawn(async move {
        let handler = DriverHandler {
            driver: driver.await?,
            port: port.clone(),
            records,
            turns,
            release_first,
        };
        let (delivery, delivery_handle) = IrqDelivery::new(port, cpu).register_handler(handler);
        let _ = handle_tx.send(delivery_handle);
        Ok(delivery.run().await)
    });
    DeliveryActor { handle, task }
}
