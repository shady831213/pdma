use crate::{DriverError, DriverResult, PortResult};
use hardware_interface_runtime::CpuLocal;
use pdma_model_contract::semantic::CommandId;
use std::{
    future::{Future, poll_fn},
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
};

#[derive(Debug)]
struct DriverAccounting {
    next_command: usize,
    free_command_slots: usize,
    command_depth: usize,
}

impl DriverAccounting {
    const fn new(command_depth: usize) -> Self {
        Self {
            next_command: 0,
            free_command_slots: command_depth,
            command_depth,
        }
    }
}

#[derive(Debug, Clone)]
enum AccountingState {
    Cpu(CpuLocal<DriverAccounting>),
    Manual(Arc<Mutex<DriverAccounting>>),
}

#[derive(Debug, Clone)]
pub(super) struct SharedAccounting {
    state: AccountingState,
    pending_rollbacks: Arc<AtomicUsize>,
}

impl SharedAccounting {
    pub(super) fn capture_active(command_depth: usize) -> DriverResult<Self> {
        Ok(Self {
            state: AccountingState::Cpu(
                CpuLocal::capture_active(DriverAccounting::new(command_depth))
                    .map_err(DriverError::from)?,
            ),
            pending_rollbacks: Arc::new(AtomicUsize::new(0)),
        })
    }

    pub(super) fn manual(command_depth: usize) -> Self {
        Self {
            state: AccountingState::Manual(Arc::new(Mutex::new(DriverAccounting::new(
                command_depth,
            )))),
            pending_rollbacks: Arc::new(AtomicUsize::new(0)),
        }
    }

    pub(super) fn prepare(&self) -> DriverResult<CommandReservation> {
        let command_id = self.with(|accounting| {
            if accounting.free_command_slots == 0 {
                return Err(DriverError::QueueFull);
            }
            let command_id = CommandId::new(accounting.next_command);
            accounting.next_command = accounting.next_command.saturating_add(1);
            accounting.free_command_slots -= 1;
            Ok(command_id)
        })??;
        Ok(CommandReservation {
            command_id,
            receipt: crate::semantic::SubmissionReceipt::new(Arc::clone(&self.pending_rollbacks)),
        })
    }

    pub(super) fn reclaim(&self) -> DriverResult<()> {
        self.with(|accounting| {
            accounting.free_command_slots = accounting
                .free_command_slots
                .saturating_add(1)
                .min(accounting.command_depth);
        })
    }

    pub(super) fn validate(&self) -> DriverResult<()> {
        self.with(|_| ())
    }

    pub(super) async fn poll_valid<F, T>(&self, future: F) -> DriverResult<T>
    where
        F: Future<Output = PortResult<T>>,
    {
        let mut future = Box::pin(future);
        poll_fn(|context| {
            // Validate before each inner poll so migration cannot cross a commit point.
            if let Err(error) = self.validate() {
                return std::task::Poll::Ready(Err(error));
            }
            future.as_mut().poll(context).map_err(DriverError::from)
        })
        .await
    }

    fn with<R>(&self, operation: impl FnOnce(&mut DriverAccounting) -> R) -> DriverResult<R> {
        match &self.state {
            AccountingState::Cpu(state) => state
                .with(|accounting| {
                    self.drain_rollbacks(accounting);
                    operation(accounting)
                })
                .map_err(DriverError::from),
            AccountingState::Manual(state) => {
                let mut state = state.lock().map_err(|_| {
                    DriverError::StateBusy(hardware_interface_runtime::CpuContextError::StateBusy)
                })?;
                self.drain_rollbacks(&mut state);
                Ok(operation(&mut state))
            }
        }
    }

    fn drain_rollbacks(&self, accounting: &mut DriverAccounting) {
        let pending_rollbacks = self.pending_rollbacks.swap(0, Ordering::AcqRel);
        accounting.free_command_slots = accounting
            .free_command_slots
            .saturating_add(pending_rollbacks)
            .min(accounting.command_depth);
    }
}

#[derive(Debug)]
pub(super) struct CommandReservation {
    command_id: CommandId,
    receipt: crate::semantic::SubmissionReceipt,
}

impl CommandReservation {
    pub(super) const fn command_id(&self) -> CommandId {
        self.command_id
    }

    pub(super) fn receipt(&self) -> crate::semantic::SubmissionReceipt {
        self.receipt.clone()
    }

    pub(super) fn disarm(self) {
        self.receipt.accept();
    }
}
