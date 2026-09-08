#![allow(missing_docs)]

pub mod assertions;
pub mod delivery;

use hardware_interface_runtime::SoftwareCpu;

use pdma_model_contract::semantic::{
    CommandId, CopyRequest, IrqNotification, IrqRoute, IrqSource, IrqVector,
};
use pdma_model_device::AsyncDevice;
use pdma_model_host::{
    AsyncDriver, DriverError, DriverIrqRecord, DriverPort, DriverResult, PortError,
    adapter::abi::{AsyncDeviceAccess, PdmaIrqCoordinator, PdmaRegisterPort},
    runtime::interrupt::PdmaIrqHandler,
};
use std::sync::Arc;
use tokio::sync::mpsc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HandlerIdentity {
    Completion,
    Event,
}

#[derive(Debug)]
pub struct HandlerObservation {
    identity: HandlerIdentity,
    call: usize,
    notification: IrqNotification,
    records: Vec<DriverIrqRecord>,
}

struct DriverHandler {
    identity: HandlerIdentity,
    calls: usize,
    source: IrqSource,
    report: mpsc::Sender<HandlerObservation>,
}

impl PdmaIrqHandler for DriverHandler {
    type Error = DriverError;

    async fn handle(
        &mut self,
        notification: IrqNotification,
        records: &[DriverIrqRecord],
    ) -> DriverResult<()> {
        if notification_source(notification) != self.source {
            return Err(DriverError::Port(PortError::UnknownIrqSource(
                notification_source(notification),
            )));
        }
        self.calls = self.calls.saturating_add(1);
        // This report channel only makes the handler order and drained records test-visible.
        let _ = self
            .report
            .send(HandlerObservation {
                identity: self.identity,
                call: self.calls,
                notification,
                records: records.to_vec(),
            })
            .await;
        Ok(())
    }
}

#[derive(Debug)]
pub struct DeliveryBootstrap {
    driver: AsyncDriver,
    port: PdmaRegisterPort<AsyncDeviceAccess>,
    completion_source: IrqSource,
    event_source: IrqSource,
}

#[derive(Debug)]
pub struct MainOutcome {
    commands: [CommandId; 3],
    sources: [IrqSource; 2],
    observations: Vec<HandlerObservation>,
    copied: Vec<u8>,
}

#[tokio::test(flavor = "current_thread")]
async fn message_irq_two_handlers_share_driver_accounting()
-> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let (device, worker) = fresh_device();
    let worker = tokio::spawn(worker.run());
    let raw_port = device.handle();
    let access = AsyncDeviceAccess::new(raw_port, PdmaIrqCoordinator::default());
    let main_access = access.clone();
    let cpu = Arc::new(SoftwareCpu::new());
    let completion_vector = IrqVector::new(65);
    let event_vector = IrqVector::new(66);
    let (report_tx, mut report_rx) = mpsc::channel(3);
    let delivery::DeliveryActor {
        bootstrap,
        ready,
        shutdown,
        join,
        task: delivery_task,
    } = delivery::spawn(delivery::DeliveryConfig {
        cpu: Arc::clone(&cpu),
        access,
        vectors: [completion_vector, event_vector],
        report: report_tx,
    });

    let main = tokio::spawn(async move {
        cpu.run_main(async move {
            let mut main_port = PdmaRegisterPort::new(main_access)?;
            let mut driver = AsyncDriver::setup_on(&mut main_port, 1, 4).await?;
            let completion_capability = driver.completion_irq_source();
            let event_capability = driver.event_irq_source();
            let completion_source = completion_capability.source();
            let event_source = event_capability.source();
            bootstrap
                .send(DeliveryBootstrap {
                    driver: driver.clone(),
                    port: main_port.clone(),
                    completion_source,
                    event_source,
                })
                .expect("delivery receives its persistent services");
            ready.await.expect("both handlers are registered");
            for (source, vector) in [
                (completion_capability, completion_vector),
                (event_capability, event_vector),
            ] {
                main_port
                    .configure_irq_route(source, IrqRoute::Message(vector))
                    .await?;
                main_port.enable_irq_source(source).await?;
                main_port.unmask_irq_source(source).await?;
            }

            let src = driver
                .allocate_buffer_on(&mut main_port, 4)
                .await
                .expect("allocation");
            let dst = driver
                .allocate_buffer_on(&mut main_port, 4)
                .await
                .expect("allocation");
            driver
                .write_buffer_on(&mut main_port, src, &[2, 4, 6, 8])
                .await?;
            let first = driver
                .submit_copy_on(&mut main_port, CopyRequest::new(src, dst, 4))
                .await?;
            let first_report = report_rx.recv().await.expect("completion handler reports");
            let fault = driver
                .submit_copy_on(&mut main_port, CopyRequest::new(src, dst, 8))
                .await?;
            let fault_report = report_rx.recv().await.expect("event handler reports");
            let final_command = driver
                .submit_copy_on(&mut main_port, CopyRequest::new(src, dst, 4))
                .await?;
            let final_report = report_rx
                .recv()
                .await
                .expect("completion handler reports again");
            let copied = driver.read_buffer_on(&main_port, dst, 4).await?;
            Ok::<_, Box<dyn std::error::Error + Send + Sync>>(MainOutcome {
                commands: [first, fault, final_command],
                sources: [completion_source, event_source],
                observations: vec![first_report, fault_report, final_report],
                copied,
            })
        })
        .await
    });

    let outcome = main.await.expect("software main joins cleanly")?;
    shutdown
        .await
        .expect("delivery publishes shutdown")
        .request_shutdown();
    let _ = join.send(());
    device.request_shutdown().await;
    let delivery_summary = delivery_task.await.expect("host delivery joins cleanly");
    let device_summary = worker.await.expect("device worker joins cleanly");

    assertions::assert_observations(&outcome, completion_vector, event_vector);
    assert_eq!(outcome.copied, vec![2, 4, 6, 8]);
    assert_eq!(delivery_summary.deliveries(), 3);
    assert_eq!(device_summary.steps, 2);
    assert_eq!(device_summary.errors, 1);
    Ok(())
}

fn fresh_device() -> (AsyncDevice, pdma_model_device::AsyncDeviceWorker) {
    AsyncDevice::new(
        pdma_model_device::PdmaMemoryProvisioning::shared(Arc::new(
            hardware_interface_runtime::SharedMemory::new(),
        )),
        pdma_model_contract::semantic::ResourceConfig::default(),
    )
}

const fn notification_source(notification: IrqNotification) -> IrqSource {
    match notification {
        IrqNotification::Line(snapshot) => snapshot.cause().source(),
        IrqNotification::Message(message) => message.source(),
    }
}
