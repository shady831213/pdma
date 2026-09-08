use super::{HandlerError, setup};
use crate::support::{ProbeAccess, ProbePort};
use hardware_interface_runtime::{
    HandlerRegistry, InterruptSource, NoopInterruptObserver, SoftwareCpu,
};
use pdma_model_contract::semantic::{IrqNotification, IrqVector};
use pdma_model_host::DriverIrqRecord;
use pdma_model_host::adapter::abi::{
    PdmaInterruptNotification, PdmaInterruptReady, PdmaInterruptSourceError,
};
use pdma_model_host::runtime::interrupt::{PdmaIrqDelivery, PdmaIrqHandler, PdmaIrqService};
use std::sync::Arc;
use tokio::sync::oneshot;

struct DropAccess {
    inner: ProbeAccess,
    dropped: Option<oneshot::Sender<()>>,
}

impl Clone for DropAccess {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            dropped: None,
        }
    }
}

impl Drop for DropAccess {
    fn drop(&mut self) {
        if let Some(dropped) = self.dropped.take() {
            let _ = dropped.send(());
        }
    }
}

impl InterruptSource for DropAccess {
    type Error = PdmaInterruptSourceError;
    type Notification = PdmaInterruptNotification;
    type Ready = PdmaInterruptReady;

    async fn wait_ready(&self) -> Result<Self::Ready, Self::Error> {
        self.inner.wait_ready().await
    }

    async fn claim(
        &mut self,
        ready: Self::Ready,
    ) -> Result<Option<Self::Notification>, Self::Error> {
        self.inner.claim(ready).await
    }
}

struct DropHandler {
    entered: Option<oneshot::Sender<()>>,
    release: Option<oneshot::Receiver<()>>,
    dropped: Option<oneshot::Sender<()>>,
}

impl Drop for DropHandler {
    fn drop(&mut self) {
        if let Some(dropped) = self.dropped.take() {
            let _ = dropped.send(());
        }
    }
}

impl PdmaIrqHandler for DropHandler {
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

pub(super) async fn drop_while_waiting_for_cpu() -> bool {
    let cpu = Arc::new(SoftwareCpu::new());
    let port = ProbePort::success();
    let driver = setup(&cpu, &port).await;
    let access = ProbeAccess::one(IrqVector::new(64), driver.completion_irq_source());
    let (source_drop_tx, source_drop_rx) = oneshot::channel();
    let (handler_drop_tx, handler_drop_rx) = oneshot::channel();
    let (handler_entered_tx, handler_entered_rx) = oneshot::channel();
    let service = PdmaIrqService::new(
        driver,
        port,
        DropHandler {
            entered: Some(handler_entered_tx),
            release: None,
            dropped: Some(handler_drop_tx),
        },
    );
    let mut registry = HandlerRegistry::new();
    assert!(registry.register(IrqVector::new(64), service).is_ok());
    let (cpu_held_tx, cpu_held_rx) = oneshot::channel();
    let (cpu_release_tx, cpu_release_rx) = oneshot::channel();
    let held_cpu = Arc::clone(&cpu);
    let holder = tokio::spawn(async move {
        held_cpu
            .run_handler_turn(async {
                let _ = cpu_held_tx.send(());
                let _ = cpu_release_rx.await;
            })
            .await;
    });
    let held = cpu_held_rx.await.is_ok();
    let task = PdmaIrqDelivery::start(
        Arc::clone(&cpu),
        registry.freeze().unwrap_or_else(|_| std::process::abort()),
        DropAccess {
            inner: access.clone(),
            dropped: Some(source_drop_tx),
        },
        Arc::new(NoopInterruptObserver),
    );
    while access.snapshot().notifications == 0 {
        tokio::task::yield_now().await;
    }
    drop(task);
    let released = cpu_release_tx.send(()).is_ok();
    let holder_joined = holder.await.is_ok();
    let handler_ran = handler_entered_rx.await.is_ok();
    let handler_dropped = handler_drop_rx.await.is_ok();
    let source_dropped = source_drop_rx.await.is_ok();
    let cpu_released = cpu.run_main(async { true }).await;
    held && released
        && holder_joined
        && handler_ran
        && handler_dropped
        && source_dropped
        && cpu_released
}

pub(super) async fn drop_while_handler_is_pending() -> bool {
    let cpu = Arc::new(SoftwareCpu::new());
    let port = ProbePort::success();
    let driver = setup(&cpu, &port).await;
    let access = ProbeAccess::one(IrqVector::new(64), driver.completion_irq_source());
    let (source_drop_tx, source_drop_rx) = oneshot::channel();
    let (handler_drop_tx, handler_drop_rx) = oneshot::channel();
    let (entered_tx, entered_rx) = oneshot::channel();
    let (release_tx, release_rx) = oneshot::channel();
    let service = PdmaIrqService::new(
        driver,
        port,
        DropHandler {
            entered: Some(entered_tx),
            release: Some(release_rx),
            dropped: Some(handler_drop_tx),
        },
    );
    let mut registry = HandlerRegistry::new();
    assert!(registry.register(IrqVector::new(64), service).is_ok());
    let task = PdmaIrqDelivery::start(
        Arc::clone(&cpu),
        registry.freeze().unwrap_or_else(|_| std::process::abort()),
        DropAccess {
            inner: access,
            dropped: Some(source_drop_tx),
        },
        Arc::new(NoopInterruptObserver),
    );
    let entered = entered_rx.await.is_ok();
    drop(task);
    let released = release_tx.send(()).is_ok();
    let handler_dropped = handler_drop_rx.await.is_ok();
    let source_dropped = source_drop_rx.await.is_ok();
    let cpu_released = cpu.run_main(async { true }).await;
    entered && released && handler_dropped && source_dropped && cpu_released
}
