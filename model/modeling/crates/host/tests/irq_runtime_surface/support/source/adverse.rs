use super::ProbeAccess;
use crate::support::{ProbePort, lock};
use hardware_interface_runtime::{InterruptHandler, InterruptSource, SoftwareCpu};
use pdma_model_contract::semantic::{DeviceEpoch, EpochIrqSource, IrqNotification, IrqVector};
use pdma_model_host::interrupt::{
    PdmaInterruptSource, PdmaIrqHandler, PdmaIrqService, PdmaIrqServiceError,
};
use pdma_model_host::register::PdmaClaimResult;
use pdma_model_host::{AsyncDriver, DriverIrqRecord};
use std::{error::Error, fmt, sync::Arc};

mod detached;
mod lifecycle;

#[derive(Debug)]
struct HandlerError;

impl fmt::Display for HandlerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("probe handler failed")
    }
}

impl Error for HandlerError {}

struct FailHandler(bool);

impl PdmaIrqHandler for FailHandler {
    type Error = HandlerError;
    async fn handle(
        &mut self,
        _notification: IrqNotification,
        _records: &[DriverIrqRecord],
    ) -> Result<(), Self::Error> {
        if self.0 { Err(HandlerError) } else { Ok(()) }
    }
}

async fn setup(cpu: &Arc<SoftwareCpu>, port: &ProbePort) -> AsyncDriver {
    let mut setup_port = port.clone();
    cpu.run_main(AsyncDriver::setup_on(&mut setup_port, 4, 4))
        .await
        .unwrap_or_else(|_| std::process::abort())
}

async fn composite_result(
    port: ProbePort,
    stale: bool,
    fail_handler: bool,
) -> (
    Result<(), PdmaIrqServiceError<HandlerError>>,
    crate::support::port::PortSnapshot,
) {
    let cpu = Arc::new(SoftwareCpu::new());
    let driver = setup(&cpu, &port).await;
    let mut capability = driver.completion_irq_source();
    if stale {
        capability = EpochIrqSource::new(DeviceEpoch::new(1), capability.source());
    }
    let access = ProbeAccess::one(IrqVector::new(64), capability);
    let mut source = PdmaInterruptSource::new(access);
    let ready = source
        .wait_ready()
        .await
        .unwrap_or_else(|_| std::process::abort());
    let claimed = source
        .claim(ready)
        .await
        .unwrap_or_else(|_| std::process::abort())
        .unwrap_or_else(|| std::process::abort());
    let mut service = PdmaIrqService::new(driver, port.clone(), FailHandler(fail_handler));
    let result = cpu.run_handler_turn(service.handle(&claimed)).await;
    (result, port.snapshot())
}

pub(super) async fn run_adverse() -> crate::support::AdverseFacts {
    let ready_access = ProbeAccess::from_claims([]);
    lock(&ready_access.state).ready_error = true;
    let ready_error = PdmaInterruptSource::new(ready_access)
        .wait_ready()
        .await
        .is_err();
    let claim_access = ProbeAccess::from_claims([PdmaClaimResult::Empty]);
    lock(&claim_access.state).claim_error = true;
    let mut claim_source = PdmaInterruptSource::new(claim_access);
    let claim_ready = claim_source
        .wait_ready()
        .await
        .unwrap_or_else(|_| std::process::abort());
    let claim_error = claim_source.claim(claim_ready).await.is_err();

    let pending_access = ProbeAccess::from_claims([PdmaClaimResult::Empty]);
    lock(&pending_access.state).pend_claim = true;
    let mut pending_source = PdmaInterruptSource::new(pending_access.clone());
    let pending_ready = pending_source
        .wait_ready()
        .await
        .unwrap_or_else(|_| std::process::abort());
    let mut pending_claim = Box::pin(pending_source.claim(pending_ready));
    let first_poll_pending = std::future::poll_fn(|context| {
        std::task::Poll::Ready(pending_claim.as_mut().poll(context).is_pending())
    })
    .await;
    drop(pending_claim);
    let precommit_cancelled = first_poll_pending && pending_access.snapshot().empty == 0;

    let (stale_result, stale_port) = composite_result(ProbePort::success(), true, false).await;
    let stale_before_drain = matches!(stale_result, Err(PdmaIrqServiceError::Validation(_)))
        && stale_port.polls == 0
        && stale_port.acks == 0;
    let (drain_result, drain_port) =
        composite_result(ProbePort::with_failures(true, false), false, false).await;
    let drain_error_no_ack =
        matches!(drain_result, Err(PdmaIrqServiceError::Drain(_))) && drain_port.acks == 0;
    let (handler_result, handler_port) = composite_result(ProbePort::success(), false, true).await;
    let handler_error_no_ack = matches!(handler_result, Err(PdmaIrqServiceError::Handler(_)))
        && handler_port.acks == 0
        && lifecycle::observer_error().await;
    let (ack_result, ack_port) =
        composite_result(ProbePort::with_failures(false, true), false, false).await;
    let ack_error = matches!(ack_result, Err(PdmaIrqServiceError::Ack(_))) && ack_port.acks == 0;

    crate::support::AdverseFacts::new([
        stale_before_drain,
        lifecycle::unregistered().await,
        ready_error,
        claim_error,
        lifecycle::shutdown_bias().await,
        precommit_cancelled,
        lifecycle::accepted_cpu_wait().await && detached::drop_while_waiting_for_cpu().await,
        lifecycle::pending_handler().await && detached::drop_while_handler_is_pending().await,
        drain_error_no_ack,
        handler_error_no_ack,
        ack_error,
    ])
}
