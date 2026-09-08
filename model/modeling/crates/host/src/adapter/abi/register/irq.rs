mod decode;

use super::{PdmaClaimError, PdmaClaimResult, PdmaRegisterAccessError};
use hardware_interface_runtime::RegisterAccess;
use pdma_model_contract::abi::mmio::{IrqCommand, IrqRegisters, MmioResult};
use std::{future::Future, sync::Arc};
use tokio::sync::{Mutex, MutexGuard, Notify};

/// Clone-shared host ownership of the IRQ command/response window.
/// Raw endpoint users must serialize their IRQ commands with this host owner.
#[derive(Clone, Debug, Default)]
pub struct PdmaIrqCoordinator {
    state: Arc<Mutex<PdmaIrqState>>,
    changed: Arc<Notify>,
}

/// Supplies the same host IRQ coordinator to ports and interrupt delivery.
pub trait PdmaIrqAccess {
    /// Returns the coordinator shared by every clone of this backend.
    fn irq_coordinator(&self) -> &PdmaIrqCoordinator;
}

/// Retained host claim progress protected by the IRQ window.
#[derive(Debug, Default)]
pub(super) struct PdmaIrqState {
    issued: Option<Issued>,
    recovered: Option<PdmaClaimResult>,
}

#[derive(Debug)]
struct Issued {
    result: Option<u64>,
    response: Vec<u64>,
}

impl PdmaIrqCoordinator {
    /// Locks the shared host IRQ window until the returned guard is dropped.
    pub(super) async fn lock(&self) -> MutexGuard<'_, PdmaIrqState> {
        self.state.lock().await
    }

    /// Resolves retained consumption before observing hardware readiness.
    pub async fn wait_ready<A, F>(&self, access: &A, ready: F) -> Result<bool, PdmaClaimError>
    where
        A: RegisterAccess<Error = PdmaRegisterAccessError> + Sync,
        F: Future<Output = bool> + Send,
    {
        let mut ready = std::pin::pin!(ready);
        loop {
            let mut changed = std::pin::pin!(self.changed.notified());
            changed.as_mut().enable();
            let mut state = self.lock().await;
            state.recover(access).await?;
            if state.recovered.is_some() {
                return Ok(true);
            }
            drop(state);
            tokio::select! {
                biased;
                () = &mut changed => {},
                ready = &mut ready => return Ok(ready),
            }
        }
    }

    /// Claims through individual accesses and retains acknowledged consumption on cancellation.
    pub async fn claim<A, F>(&self, access: &A, ready: F) -> Result<PdmaClaimResult, PdmaClaimError>
    where
        A: RegisterAccess<Error = PdmaRegisterAccessError> + Sync,
        F: Future<Output = bool> + Send,
    {
        let mut state = self.lock().await;
        state.recover(access).await?;
        if let Some(result) = state.recovered.take() {
            return Ok(result);
        }
        drop(state);
        if !self.wait_ready(access, ready).await? {
            return Ok(PdmaClaimResult::Shutdown);
        }
        let mut state = self.lock().await;
        state.recover(access).await?;
        if let Some(result) = state.recovered.take() {
            return Ok(result);
        }
        decode::write_claim(access, IrqCommand::ClaimNotification).await?;
        state.issued = Some(Issued {
            result: None,
            response: Vec::new(),
        });
        self.changed.notify_waiters();
        state.recover(access).await?;
        state
            .recovered
            .take()
            .ok_or(PdmaClaimError::MalformedResponse)
    }
}

impl PdmaIrqState {
    /// Captures an issued claim before another IRQ command can overwrite its response.
    pub(super) async fn recover<A>(&mut self, access: &A) -> Result<(), PdmaClaimError>
    where
        A: RegisterAccess<Error = PdmaRegisterAccessError> + Sync,
    {
        let Some(issued) = self.issued.as_mut() else {
            return Ok(());
        };
        let result = decode::read(access, IrqRegisters::RESULT.offset()).await?;
        if let Some(previous) = issued.result {
            if previous != result {
                self.issued = None;
                return Err(PdmaClaimError::CommandResult { code: result });
            }
        }
        issued.result = Some(result);
        if result == u64::from(MmioResult::Ok.code()) {
            for offset in decode::response_offsets().skip(issued.response.len()) {
                issued.response.push(decode::read(access, offset).await?);
            }
        }
        let current_result = decode::read(access, IrqRegisters::RESULT.offset()).await?;
        let outcome = if current_result != result {
            Err(PdmaClaimError::CommandResult {
                code: current_result,
            })
        } else if result == u64::from(MmioResult::Empty.code()) {
            Ok(PdmaClaimResult::Empty)
        } else if result == u64::from(MmioResult::Ok.code()) {
            decode::notification(&issued.response)
                .map(|notification| PdmaClaimResult::Notification { notification })
        } else {
            Err(PdmaClaimError::CommandResult { code: result })
        };
        self.issued = None;
        self.recovered = Some(outcome?);
        Ok(())
    }
}
