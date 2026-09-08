mod assertions;
mod recording;

pub use assertions::assert_outcome;
pub use recording::{MemoryAccessRecord, RecordingMemoryAccess, RecordingRegisterObserver};

use hardware_interface_runtime::{
    CpuLocal, HandlerRegistry, InterruptShutdown, InterruptSummary, NoopInterruptObserver,
    RegisterObservation, SoftwareCpu,
};
use pdma_model_contract::semantic::{CommandId, IrqNotification, IrqSource, IrqVector, TraceEvent};
use pdma_model_device::{AsyncDevice, AsyncDeviceWorker};
use pdma_model_host::QueueConfigHandle;
use pdma_model_host::{
    AsyncDriver, DriverIrqRecord,
    adapter::abi::{PdmaRegisterPort, RegisterQueueStatus},
    runtime::interrupt::{PdmaIrqDelivery, PdmaIrqHandler, PdmaIrqService},
};
use std::sync::Arc;
use tokio::{sync::oneshot, task::JoinHandle};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SoftwarePhase {
    MainBeforeSubmit,
    Handler,
    MainAfterHandler,
}

#[derive(Debug)]
pub struct HandlerReport {
    notification: IrqNotification,
    records: Vec<DriverIrqRecord>,
}

#[derive(Debug)]
pub struct LinearCopyHandler {
    pub phase: CpuLocal<SoftwarePhase>,
    pub report: Option<oneshot::Sender<HandlerReport>>,
}

impl PdmaIrqHandler for LinearCopyHandler {
    type Error = hardware_interface_runtime::CpuContextError;

    async fn handle(
        &mut self,
        notification: IrqNotification,
        records: &[DriverIrqRecord],
    ) -> Result<(), Self::Error> {
        self.phase.with(|phase| *phase = SoftwarePhase::Handler)?;
        // This report is scenario evidence, not part of IRQ handler mechanics.
        if let Some(report) = self.report.take() {
            drop(report.send(HandlerReport {
                notification,
                records: records.to_vec(),
            }));
        }
        Ok(())
    }
}

#[derive(Debug)]
pub struct DeliveryConfig {
    pub cpu: Arc<SoftwareCpu>,
    pub access: RecordingMemoryAccess,
    pub phase: CpuLocal<SoftwarePhase>,
    pub vector: IrqVector,
    pub report: oneshot::Sender<HandlerReport>,
}

#[derive(Debug)]
pub struct DeliveryActor {
    pub bootstrap: oneshot::Sender<(AsyncDriver, PdmaRegisterPort<RecordingMemoryAccess>)>,
    pub ready: oneshot::Receiver<()>,
    pub shutdown: oneshot::Receiver<InterruptShutdown>,
    pub join: oneshot::Sender<()>,
    pub task: JoinHandle<InterruptSummary>,
}

pub fn fresh_device(observer: Arc<RecordingRegisterObserver>) -> (AsyncDevice, AsyncDeviceWorker) {
    AsyncDevice::new_observed(
        pdma_model_device::PdmaMemoryProvisioning::shared(Arc::new(
            hardware_interface_runtime::SharedMemory::new(),
        )),
        pdma_model_contract::semantic::ResourceConfig::default(),
        observer,
    )
}

pub fn spawn_delivery(config: DeliveryConfig) -> DeliveryActor {
    let (bootstrap, bootstrap_rx) = oneshot::channel();
    let (ready_tx, ready) = oneshot::channel();
    let (shutdown_tx, shutdown) = oneshot::channel();
    let (join, join_rx) = oneshot::channel();
    let task = tokio::spawn(async move {
        let (driver, port) = bootstrap_rx.await.unwrap_or_else(|_| std::process::abort());
        let handler = LinearCopyHandler {
            phase: config.phase,
            report: Some(config.report),
        };
        let mut registry = HandlerRegistry::new();
        if registry
            .register(config.vector, PdmaIrqService::new(driver, port, handler))
            .is_err()
        {
            std::process::abort();
        }
        let delivery = PdmaIrqDelivery::start(
            config.cpu,
            registry.freeze().unwrap_or_else(|_| std::process::abort()),
            config.access,
            Arc::new(NoopInterruptObserver),
        );
        let _ = shutdown_tx.send(delivery.shutdown_handle());
        let _ = ready_tx.send(());
        let _ = join_rx.await;
        delivery
            .shutdown_and_join()
            .await
            .unwrap_or_else(|_| std::process::abort())
    });
    DeliveryActor {
        bootstrap,
        ready,
        shutdown,
        join,
        task,
    }
}

#[derive(Debug)]
pub struct MainOutcome {
    pub command: CommandId,
    pub queue: QueueConfigHandle,
    pub source: IrqSource,
    pub report: HandlerReport,
    pub copied: Vec<u8>,
    pub phase: SoftwarePhase,
    pub queue_status: RegisterQueueStatus,
}

#[derive(Debug)]
pub struct ScenarioResult {
    pub vector: IrqVector,
    pub outcome: MainOutcome,
    pub queue_status: RegisterQueueStatus,
    pub deliveries: usize,
    pub device_steps: usize,
    pub device_trace: Vec<TraceEvent>,
    pub mmio_accesses: Vec<RegisterObservation>,
    pub memory_accesses: Vec<MemoryAccessRecord>,
}

pub async fn start_trace(port: &pdma_model_device::AsyncDeviceHandle) {
    port.enable_tracing().await;
    port.clear_trace().await;
}
