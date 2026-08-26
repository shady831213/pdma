use crate::ModelError;
use crate::ModelResult;
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
    pub(super) fn capture_active(command_depth: usize) -> ModelResult<Self> {
        Ok(Self {
            state: AccountingState::Cpu(
                CpuLocal::capture_active(DriverAccounting::new(command_depth))
                    .map_err(ModelError::from)?,
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

    pub(super) fn prepare(&self) -> ModelResult<CommandReservation> {
        let command_id = self.with(|accounting| {
            if accounting.free_command_slots == 0 {
                return Err(ModelError::DriverQueueFull);
            }
            let command_id = CommandId::new(accounting.next_command);
            accounting.next_command = accounting.next_command.saturating_add(1);
            accounting.free_command_slots -= 1;
            Ok(command_id)
        })??;
        Ok(CommandReservation {
            command_id,
            pending_rollbacks: Arc::clone(&self.pending_rollbacks),
            armed: true,
        })
    }

    pub(super) fn reclaim(&self) -> ModelResult<()> {
        self.with(|accounting| {
            accounting.free_command_slots = accounting
                .free_command_slots
                .saturating_add(1)
                .min(accounting.command_depth);
        })
    }

    pub(super) fn validate(&self) -> ModelResult<()> {
        self.with(|_| ())
    }

    pub(super) async fn poll_valid<F, T>(&self, future: F) -> ModelResult<T>
    where
        F: Future<Output = ModelResult<T>>,
    {
        let mut future = Box::pin(future);
        poll_fn(|context| {
            // Validate before each inner poll so migration cannot cross a commit point.
            if let Err(error) = self.validate() {
                return std::task::Poll::Ready(Err(error));
            }
            future.as_mut().poll(context)
        })
        .await
    }

    fn with<R>(&self, operation: impl FnOnce(&mut DriverAccounting) -> R) -> ModelResult<R> {
        match &self.state {
            AccountingState::Cpu(state) => state
                .with(|accounting| {
                    self.drain_rollbacks(accounting);
                    operation(accounting)
                })
                .map_err(ModelError::from),
            AccountingState::Manual(state) => {
                let mut state = state.lock().map_err(|_| ModelError::SoftwareCpuStateBusy)?;
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
    pending_rollbacks: Arc<AtomicUsize>,
    armed: bool,
}

impl CommandReservation {
    pub(super) const fn command_id(&self) -> CommandId {
        self.command_id
    }

    pub(super) fn disarm(mut self) {
        self.armed = false;
    }
}

impl Drop for CommandReservation {
    fn drop(&mut self) {
        if self.armed {
            // Pre-acceptance cancellation defers rollback to canonical accounting.
            self.pending_rollbacks.fetch_add(1, Ordering::Release);
        }
    }
}
