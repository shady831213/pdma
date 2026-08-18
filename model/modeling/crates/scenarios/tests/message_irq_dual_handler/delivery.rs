use super::{DeliveryBootstrap, DriverHandler, HandlerIdentity, HandlerObservation};
use hardware_interface_runtime::{
    HandlerRegistry, InterruptShutdown, InterruptSummary, NoopInterruptObserver, SoftwareCpu,
};
use pdma_model_contract::semantic::IrqVector;
use pdma_model_host::{
    adapters::AsyncDeviceAccess,
    compat::AsyncDevicePort,
    interrupt::{PdmaInterruptSource, PdmaIrqDelivery, PdmaIrqService},
};
use std::sync::Arc;
use tokio::{
    sync::{mpsc, oneshot},
    task::JoinHandle,
};

#[derive(Debug)]
pub struct DeliveryActor {
    pub bootstrap: oneshot::Sender<DeliveryBootstrap>,
    pub ready: oneshot::Receiver<()>,
    pub shutdown: oneshot::Receiver<InterruptShutdown>,
    pub join: oneshot::Sender<()>,
    pub task: JoinHandle<InterruptSummary>,
}

#[derive(Debug)]
pub struct DeliveryConfig {
    pub cpu: Arc<SoftwareCpu>,
    pub access: AsyncDeviceAccess,
    pub port: AsyncDevicePort,
    pub vectors: [IrqVector; 2],
    pub report: mpsc::Sender<HandlerObservation>,
}

pub fn spawn(config: DeliveryConfig) -> DeliveryActor {
    let (bootstrap, bootstrap_rx) = oneshot::channel();
    let (ready_tx, ready) = oneshot::channel();
    let (shutdown_tx, shutdown) = oneshot::channel();
    let (join, join_rx) = oneshot::channel();
    let task = tokio::spawn(async move {
        let state: DeliveryBootstrap = bootstrap_rx.await.unwrap_or_else(|_| std::process::abort());
        let mut registry = HandlerRegistry::new();
        let completion = DriverHandler {
            identity: HandlerIdentity::Completion,
            calls: 0,
            source: state.completion_source,
            report: config.report.clone(),
        };
        let event = DriverHandler {
            identity: HandlerIdentity::Event,
            calls: 0,
            source: state.event_source,
            report: config.report,
        };
        if registry
            .register(
                config.vectors[0],
                PdmaIrqService::new(state.driver.clone(), config.port.clone(), completion),
            )
            .is_err()
        {
            std::process::abort();
        }
        if registry
            .register(
                config.vectors[1],
                PdmaIrqService::new(state.driver, config.port, event),
            )
            .is_err()
        {
            std::process::abort();
        }
        let delivery = PdmaIrqDelivery::start(
            config.cpu,
            registry.freeze().unwrap_or_else(|_| std::process::abort()),
            PdmaInterruptSource::new(config.access),
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
