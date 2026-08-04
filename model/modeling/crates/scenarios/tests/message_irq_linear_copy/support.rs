#[path = "assertions.rs"]
mod assertions;
#[path = "recording.rs"]
mod recording;

pub use assertions::assert_outcome;
pub use recording::{MmioAccessRecord, RecordingMmioTransport, ScenarioBus};

use pdma_model_contract::{
    CommandId, IrqNotification, IrqSource, IrqVector, ModelResult, QueueConfigHandle, TraceEvent,
};
use pdma_model_host::{
    AsyncDriver, BusDevicePort, BusQueueStatus, CpuLocal, DriverIrqRecord, StatefulIrqHandler,
};
use tokio::sync::oneshot;

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
    pub driver: AsyncDriver,
    pub port: BusDevicePort<ScenarioBus>,
    pub phase: CpuLocal<SoftwarePhase>,
    pub report: Option<oneshot::Sender<HandlerReport>>,
}

impl StatefulIrqHandler for LinearCopyHandler {
    async fn handle(&mut self, notification: IrqNotification) -> ModelResult<()> {
        self.phase.with(|phase| *phase = SoftwarePhase::Handler)?;
        let records = self
            .driver
            .service_irq_on(&mut self.port, notification)
            .await?;
        // This report is scenario evidence, not part of IRQ handler mechanics.
        if let Some(report) = self.report.take() {
            drop(report.send(HandlerReport {
                notification,
                records,
            }));
        }
        Ok(())
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
}

#[derive(Debug)]
pub struct ScenarioResult {
    pub vector: IrqVector,
    pub outcome: MainOutcome,
    pub queue_status: BusQueueStatus,
    pub deliveries: usize,
    pub device_steps: usize,
    pub device_trace: Vec<TraceEvent>,
    pub mmio_accesses: Vec<MmioAccessRecord>,
}

pub async fn start_trace(port: &pdma_model_device::AsyncDeviceHandle) {
    port.enable_tracing().await;
    port.clear_trace().await;
}
