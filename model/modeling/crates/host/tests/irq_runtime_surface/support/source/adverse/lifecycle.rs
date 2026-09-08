use super::{FailHandler, HandlerError, setup};
use crate::support::{PersistentHandler, ProbeAccess, ProbePort};
use hardware_interface_runtime::{
    HandlerRegistry, InterruptDispatcher, NoopInterruptObserver, SoftwareCpu,
};
use pdma_model_contract::semantic::{IrqNotification, IrqVector};
use pdma_model_host::DriverIrqRecord;
use pdma_model_host::runtime::interrupt::{
    PdmaInterruptTrace, PdmaIrqDelivery, PdmaIrqHandler, PdmaIrqService,
};
use std::sync::{Arc, Mutex};

struct PendingHandler {
    entered: Option<tokio::sync::oneshot::Sender<()>>,
    release: Option<tokio::sync::oneshot::Receiver<()>>,
}

impl PdmaIrqHandler for PendingHandler {
    type Error = HandlerError;

    async fn handle(
        &mut self,
        _notification: IrqNotification,
        _records: &[DriverIrqRecord],
    ) -> Result<(), Self::Error> {
        if let Some(entered) = self.entered.take() {
            let _ = entered.send(());
        }
        if let Some(release) = self.release.take() {
            let _ = release.await;
        }
        Ok(())
    }
}

pub(super) async fn unregistered() -> bool {
    let cpu = Arc::new(SoftwareCpu::new());
    let port = ProbePort::success();
    let driver = setup(&cpu, &port).await;
    let access = ProbeAccess::one(IrqVector::new(65), driver.completion_irq_source());
    let service = PdmaIrqService::new(
        driver,
        port,
        PersistentHandler::new(Arc::new(Mutex::new(crate::support::Facts::default()))),
    );
    let mut registry = HandlerRegistry::new();
    assert!(registry.register(IrqVector::new(64), service).is_ok());
    let task = PdmaIrqDelivery::start(
        cpu,
        registry.freeze().unwrap_or_else(|_| std::process::abort()),
        access.clone(),
        Arc::new(NoopInterruptObserver),
    );
    while access.snapshot().notifications == 0 {
        tokio::task::yield_now().await;
    }
    task.shutdown_and_join().await.is_err()
}

pub(super) async fn shutdown_bias() -> bool {
    let cpu = Arc::new(SoftwareCpu::new());
    let port = ProbePort::success();
    let driver = setup(&cpu, &port).await;
    let access = ProbeAccess::one(IrqVector::new(64), driver.completion_irq_source());
    let service = PdmaIrqService::new(driver, port, FailHandler(false));
    let mut registry = HandlerRegistry::new();
    assert!(registry.register(IrqVector::new(64), service).is_ok());
    let task = InterruptDispatcher::start(
        cpu,
        registry.freeze().unwrap_or_else(|_| std::process::abort()),
        access.clone(),
        Arc::new(NoopInterruptObserver),
    );
    task.shutdown_handle().request_shutdown();
    task.shutdown_and_join()
        .await
        .is_ok_and(|summary| summary.deliveries() == 0 && access.snapshot().notifications == 0)
}

pub(super) async fn accepted_cpu_wait() -> bool {
    let cpu = Arc::new(SoftwareCpu::new());
    let port = ProbePort::success();
    let driver = setup(&cpu, &port).await;
    let access = ProbeAccess::one(IrqVector::new(64), driver.completion_irq_source());
    let service = PdmaIrqService::new(driver, port, FailHandler(false));
    let mut registry = HandlerRegistry::new();
    assert!(registry.register(IrqVector::new(64), service).is_ok());
    let (release_tx, release_rx) = tokio::sync::oneshot::channel();
    let held_cpu = Arc::clone(&cpu);
    let holder = tokio::spawn(async move {
        held_cpu
            .run_handler_turn(async {
                let _ = release_rx.await;
            })
            .await;
    });
    tokio::task::yield_now().await;
    let task = PdmaIrqDelivery::start(
        cpu,
        registry.freeze().unwrap_or_else(|_| std::process::abort()),
        access.clone(),
        Arc::new(NoopInterruptObserver),
    );
    while access.snapshot().notifications == 0 {
        tokio::task::yield_now().await;
    }
    let join = tokio::spawn(task.shutdown_and_join());
    tokio::task::yield_now().await;
    let waited = !join.is_finished();
    let _ = release_tx.send(());
    waited && holder.await.is_ok() && join.await.ok().and_then(Result::ok).is_some()
}

pub(super) async fn pending_handler() -> bool {
    let cpu = Arc::new(SoftwareCpu::new());
    let port = ProbePort::success();
    let driver = setup(&cpu, &port).await;
    let access = ProbeAccess::one(IrqVector::new(64), driver.completion_irq_source());
    let (entered_tx, entered_rx) = tokio::sync::oneshot::channel();
    let (release_tx, release_rx) = tokio::sync::oneshot::channel();
    let service = PdmaIrqService::new(
        driver,
        port,
        PendingHandler {
            entered: Some(entered_tx),
            release: Some(release_rx),
        },
    );
    let mut registry = HandlerRegistry::new();
    assert!(registry.register(IrqVector::new(64), service).is_ok());
    let task = PdmaIrqDelivery::start(
        cpu,
        registry.freeze().unwrap_or_else(|_| std::process::abort()),
        access,
        Arc::new(NoopInterruptObserver),
    );
    let entered = entered_rx.await.is_ok();
    let join = tokio::spawn(task.shutdown_and_join());
    tokio::task::yield_now().await;
    let waited = !join.is_finished();
    let released = release_tx.send(()).is_ok();
    entered && waited && released && join.await.ok().and_then(Result::ok).is_some()
}

pub(super) async fn observer_error() -> bool {
    let cpu = Arc::new(SoftwareCpu::new());
    let port = ProbePort::success();
    let driver = setup(&cpu, &port).await;
    let access = ProbeAccess::one(IrqVector::new(64), driver.completion_irq_source());
    let service = PdmaIrqService::new(driver, port, FailHandler(true));
    let mut registry = HandlerRegistry::new();
    assert!(registry.register(IrqVector::new(64), service).is_ok());
    let trace = Arc::new(PdmaInterruptTrace::default());
    let task = PdmaIrqDelivery::start(
        cpu,
        registry.freeze().unwrap_or_else(|_| std::process::abort()),
        access.clone(),
        trace.clone(),
    );
    while access.snapshot().notifications == 0 {
        tokio::task::yield_now().await;
    }
    let failed = task.shutdown_and_join().await.is_err();
    let events = trace.events();
    failed
        && matches!(
            events.as_slice(),
            [started, completed]
                if started.kind() == pdma_model_contract::semantic::TraceEventKind::IrqHandlerStarted
                    && completed.kind()
                        == pdma_model_contract::semantic::TraceEventKind::IrqHandlerCompleted
                    && completed.detail() == "Error"
        )
}
