use hardware_interface_runtime::{CpuLocal, InterruptShutdown, InterruptSummary, SoftwareCpu};
use pdma_model_contract::semantic::{IrqSource, IrqVector};
use pdma_model_host::{
    AsyncDriver,
    adapter::abi::{AsyncDeviceAccess, PdmaRegisterPort},
    runtime::interrupt::PdmaInterruptTrace,
};
use std::sync::Arc;
use tokio::{
    sync::{mpsc, oneshot},
    task::JoinHandle,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HandlerIdentity {
    A,
    B,
    Shared,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecordKind {
    Completion,
    Event,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Milestone {
    MainSubmitA(usize),
    MainSubmitB(usize),
    HandlerStarted(HandlerIdentity, usize),
    HandlerCompleted(HandlerIdentity, usize),
    MainResumed(usize),
    MainRetainedSubmit,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HandlerObservation {
    pub identity: HandlerIdentity,
    pub call: usize,
    pub vector: IrqVector,
    pub source: IrqSource,
    pub records: Vec<RecordKind>,
    pub entry_depth: usize,
    pub exit_depth: usize,
}

#[derive(Debug)]
pub struct DriverBootstrap {
    pub port: PdmaRegisterPort<AsyncDeviceAccess>,
    pub driver: AsyncDriver,
    pub completion_source: IrqSource,
    pub event_source: IrqSource,
}

#[derive(Debug)]
pub struct HandlerChannels {
    pub observations: mpsc::Sender<HandlerObservation>,
    pub milestones: mpsc::Sender<Milestone>,
    pub done: mpsc::Sender<()>,
    pub releases: Option<mpsc::Receiver<()>>,
}

#[derive(Debug)]
pub struct DeliveryHandle {
    pub(super) shutdown: InterruptShutdown,
    pub(super) trace: Arc<PdmaInterruptTrace>,
    pub(super) join: oneshot::Sender<()>,
}

impl DeliveryHandle {
    pub fn request_shutdown(self) {
        self.shutdown.request_shutdown();
        let _ = self.join.send(());
    }

    pub fn trace_events(&self) -> Vec<pdma_model_contract::semantic::TraceEvent> {
        self.trace.events()
    }
}

#[derive(Debug)]
pub struct DeliveryActor {
    pub handle: oneshot::Receiver<DeliveryHandle>,
    pub task: JoinHandle<InterruptSummary>,
}

#[derive(Debug)]
pub struct DistinctDelivery {
    pub access: AsyncDeviceAccess,
    pub cpu: Arc<SoftwareCpu>,
    pub bootstrap: oneshot::Receiver<DriverBootstrap>,
    pub ready: oneshot::Sender<()>,
    pub depth: CpuLocal<usize>,
    pub vector_a: IrqVector,
    pub vector_b: IrqVector,
    pub handler_a: HandlerChannels,
    pub handler_b: HandlerChannels,
}

#[derive(Debug)]
pub struct SharedDelivery {
    pub access: AsyncDeviceAccess,
    pub cpu: Arc<SoftwareCpu>,
    pub bootstrap: oneshot::Receiver<DriverBootstrap>,
    pub ready: oneshot::Sender<()>,
    pub depth: CpuLocal<usize>,
    pub vector: IrqVector,
    pub handler: HandlerChannels,
}
