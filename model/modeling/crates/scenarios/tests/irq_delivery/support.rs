use pdma_model_contract::{IrqNotification, IrqSource, IrqVector, ModelError, ModelResult};
use pdma_model_device::AsyncDeviceHandle;
use pdma_model_host::{
    AsyncDriver, CpuLocal, DriverIrqRecord, IrqDelivery, IrqDeliveryHandle, IrqDeliverySummary,
    IrqHandler, IrqHandlerFuture, SoftwareCpu,
};
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
pub struct DeliveryActor {
    pub handle: oneshot::Receiver<IrqDeliveryHandle>,
    pub task: JoinHandle<Result<ModelResult<IrqDeliverySummary>, oneshot::error::RecvError>>,
}

#[derive(Debug)]
pub struct DistinctDelivery {
    pub port: AsyncDeviceHandle,
    pub cpu: SoftwareCpu,
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
    pub port: AsyncDeviceHandle,
    pub cpu: SoftwareCpu,
    pub bootstrap: oneshot::Receiver<DriverBootstrap>,
    pub ready: oneshot::Sender<()>,
    pub depth: CpuLocal<usize>,
    pub vector: IrqVector,
    pub handler: HandlerChannels,
}

struct DriverHandler {
    identity: HandlerIdentity,
    calls: usize,
    driver: AsyncDriver,
    port: AsyncDeviceHandle,
    depth: CpuLocal<usize>,
    sources: HandlerSources,
    channels: HandlerChannels,
}

enum HandlerSources {
    One(IrqSource),
    Both(IrqSource, IrqSource),
}

impl HandlerSources {
    fn contains(&self, source: IrqSource) -> bool {
        match self {
            Self::One(expected) => source == *expected,
            Self::Both(first, second) => source == *first || source == *second,
        }
    }
}

impl DriverHandler {
    const fn new(
        identity: HandlerIdentity,
        driver: AsyncDriver,
        port: AsyncDeviceHandle,
        depth: CpuLocal<usize>,
        sources: HandlerSources,
        channels: HandlerChannels,
    ) -> Self {
        Self {
            identity,
            calls: 0,
            driver,
            port,
            depth,
            sources,
            channels,
        }
    }
}

impl IrqHandler for DriverHandler {
    fn handle(&mut self, notification: IrqNotification) -> IrqHandlerFuture<'_> {
        Box::pin(async move {
            self.calls = self.calls.saturating_add(1);
            let call = self.calls;
            let source = match notification {
                IrqNotification::Line(snapshot) => snapshot.cause().source(),
                IrqNotification::Message(message) => message.source(),
            };
            if !self.sources.contains(source) {
                return Err(ModelError::UnknownIrqSource(source));
            }
            let entry_depth = self.depth.with(|depth| {
                *depth = depth.saturating_add(1);
                *depth
            })?;
            let _ = self
                .channels
                .milestones
                .send(Milestone::HandlerStarted(self.identity, call))
                .await;
            if let Some(releases) = self.channels.releases.as_mut() {
                let _ = releases.recv().await;
            }
            let records = self
                .driver
                .service_irq_on(&mut self.port, notification)
                .await;
            let exit_depth = self.depth.with(|depth| {
                *depth = depth.saturating_sub(1);
                *depth
            })?;
            let records = records?
                .into_iter()
                .map(|record| match record {
                    DriverIrqRecord::Completion(_) => RecordKind::Completion,
                    DriverIrqRecord::Event(_) => RecordKind::Event,
                })
                .collect();
            let observation = HandlerObservation {
                identity: self.identity,
                call,
                vector: notification.vector(),
                source,
                records,
                entry_depth,
                exit_depth,
            };
            let _ = self.channels.observations.send(observation).await;
            let _ = self
                .channels
                .milestones
                .send(Milestone::HandlerCompleted(self.identity, call))
                .await;
            let _ = self.channels.done.send(()).await;
            Ok(())
        })
    }
}

pub fn spawn_distinct_delivery(config: DistinctDelivery) -> DeliveryActor {
    let (handle_tx, handle) = oneshot::channel();
    let task = tokio::spawn(async move {
        let bootstrap = config.bootstrap.await?;
        let handler_a = DriverHandler::new(
            HandlerIdentity::A,
            bootstrap.driver.clone(),
            config.port.clone(),
            config.depth.clone(),
            HandlerSources::One(bootstrap.completion_source),
            config.handler_a,
        );
        let handler_b = DriverHandler::new(
            HandlerIdentity::B,
            bootstrap.driver,
            config.port.clone(),
            config.depth,
            HandlerSources::One(bootstrap.event_source),
            config.handler_b,
        );
        let (mut delivery, delivery_handle) =
            IrqDelivery::new(config.port, config.cpu).register_handler(config.vector_a, handler_a);
        if let Err(error) = delivery.register_handler(config.vector_b, handler_b) {
            return Ok(Err(error));
        }
        let _ = handle_tx.send(delivery_handle);
        let _ = config.ready.send(());
        Ok(delivery.run().await)
    });
    DeliveryActor { handle, task }
}

pub fn spawn_shared_delivery(config: SharedDelivery) -> DeliveryActor {
    let (handle_tx, handle) = oneshot::channel();
    let task = tokio::spawn(async move {
        let bootstrap = config.bootstrap.await?;
        let handler = DriverHandler::new(
            HandlerIdentity::Shared,
            bootstrap.driver,
            config.port.clone(),
            config.depth,
            HandlerSources::Both(bootstrap.completion_source, bootstrap.event_source),
            config.handler,
        );
        let (delivery, delivery_handle) =
            IrqDelivery::new(config.port, config.cpu).register_handler(config.vector, handler);
        let _ = handle_tx.send(delivery_handle);
        let _ = config.ready.send(());
        Ok(delivery.run().await)
    });
    DeliveryActor { handle, task }
}
