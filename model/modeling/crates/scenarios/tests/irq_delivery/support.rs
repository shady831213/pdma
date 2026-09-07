mod types;

pub use types::*;

use hardware_interface_runtime::{CpuLocal, HandlerRegistry, InterruptLifecycleObserver};
use pdma_model_contract::semantic::{IrqNotification, IrqSource};
use pdma_model_host::{
    DriverError, DriverIrqRecord, PortError,
    adapter::abi::PdmaInterruptSource,
    runtime::interrupt::{PdmaInterruptTrace, PdmaIrqDelivery, PdmaIrqHandler, PdmaIrqService},
};
use std::sync::Arc;
use tokio::sync::oneshot;

struct DriverHandler {
    identity: HandlerIdentity,
    calls: usize,
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
        depth: CpuLocal<usize>,
        sources: HandlerSources,
        channels: HandlerChannels,
    ) -> Self {
        Self {
            identity,
            calls: 0,
            depth,
            sources,
            channels,
        }
    }
}

impl PdmaIrqHandler for DriverHandler {
    type Error = DriverError;

    async fn handle(
        &mut self,
        notification: IrqNotification,
        records: &[DriverIrqRecord],
    ) -> Result<(), Self::Error> {
        self.calls = self.calls.saturating_add(1);
        let call = self.calls;
        let source = match notification {
            IrqNotification::Line(snapshot) => snapshot.cause().source(),
            IrqNotification::Message(message) => message.source(),
        };
        if !self.sources.contains(source) {
            return Err(DriverError::Port(PortError::UnknownIrqSource(source)));
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
        let exit_depth = self.depth.with(|depth| {
            *depth = depth.saturating_sub(1);
            *depth
        })?;
        let records = records
            .iter()
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
    }
}

pub fn spawn_distinct_delivery(config: DistinctDelivery) -> DeliveryActor {
    let (handle_tx, handle) = oneshot::channel();
    let task = tokio::spawn(async move {
        let bootstrap = config
            .bootstrap
            .await
            .unwrap_or_else(|_| std::process::abort());
        let handler_a = DriverHandler::new(
            HandlerIdentity::A,
            config.depth.clone(),
            HandlerSources::One(bootstrap.completion_source),
            config.handler_a,
        );
        let handler_b = DriverHandler::new(
            HandlerIdentity::B,
            config.depth,
            HandlerSources::One(bootstrap.event_source),
            config.handler_b,
        );
        let mut registry = HandlerRegistry::new();
        if registry
            .register(
                config.vector_a,
                PdmaIrqService::new(bootstrap.driver.clone(), config.port.clone(), handler_a),
            )
            .is_err()
        {
            std::process::abort();
        }
        if registry
            .register(
                config.vector_b,
                PdmaIrqService::new(bootstrap.driver, config.port, handler_b),
            )
            .is_err()
        {
            std::process::abort();
        }
        let trace = Arc::new(PdmaInterruptTrace::default());
        let observer: Arc<dyn InterruptLifecycleObserver<_>> = trace.clone();
        let delivery = PdmaIrqDelivery::start(
            config.cpu,
            registry.freeze().unwrap_or_else(|_| std::process::abort()),
            PdmaInterruptSource::new(config.access),
            observer,
        );
        let (join_tx, join_rx) = oneshot::channel();
        let _ = handle_tx.send(DeliveryHandle {
            shutdown: delivery.shutdown_handle(),
            trace,
            join: join_tx,
        });
        let _ = config.ready.send(());
        let _ = join_rx.await;
        delivery
            .shutdown_and_join()
            .await
            .unwrap_or_else(|_| std::process::abort())
    });
    DeliveryActor { handle, task }
}

pub fn spawn_shared_delivery(config: SharedDelivery) -> DeliveryActor {
    let (handle_tx, handle) = oneshot::channel();
    let task = tokio::spawn(async move {
        let bootstrap = config
            .bootstrap
            .await
            .unwrap_or_else(|_| std::process::abort());
        let handler = DriverHandler::new(
            HandlerIdentity::Shared,
            config.depth,
            HandlerSources::Both(bootstrap.completion_source, bootstrap.event_source),
            config.handler,
        );
        let mut registry = HandlerRegistry::new();
        if registry
            .register(
                config.vector,
                PdmaIrqService::new(bootstrap.driver, config.port, handler),
            )
            .is_err()
        {
            std::process::abort();
        }
        let trace = Arc::new(PdmaInterruptTrace::default());
        let observer: Arc<dyn InterruptLifecycleObserver<_>> = trace.clone();
        let delivery = PdmaIrqDelivery::start(
            config.cpu,
            registry.freeze().unwrap_or_else(|_| std::process::abort()),
            PdmaInterruptSource::new(config.access),
            observer,
        );
        let (join_tx, join_rx) = oneshot::channel();
        let _ = handle_tx.send(DeliveryHandle {
            shutdown: delivery.shutdown_handle(),
            trace,
            join: join_tx,
        });
        let _ = config.ready.send(());
        let _ = join_rx.await;
        delivery
            .shutdown_and_join()
            .await
            .unwrap_or_else(|_| std::process::abort())
    });
    DeliveryActor { handle, task }
}
