//! Driver-port probe for composite drain and ACK state.

use pdma_model_contract::semantic::{
    BufferId, CommandId, CompletionRecord, CompletionRingId, CompletionStatus, DeviceEpoch,
    EpochIrqSource, EventErrorKind, EventRecord, ExternalCommand, IrqRoute, IrqSnapshot,
    QueueConfig, QueueConfigHandle, QueueDirectoryEntry, QueueId,
};
use pdma_model_host::{DriverPort, PortError};
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
};

use super::lock;

#[derive(Clone, Debug)]
/// Clone-shared driver port used by the terminal surface.
pub struct ProbePort {
    state: Arc<Mutex<State>>,
}

#[derive(Debug)]
struct State {
    completions: VecDeque<CompletionRecord>,
    events: VecDeque<EventRecord>,
    polls: usize,
    acks: usize,
    fail_poll: bool,
    fail_ack: bool,
}

#[derive(Clone, Copy, Debug)]
/// Stable state observed after a composite turn.
pub struct PortSnapshot {
    /// Number of completion/event poll operations.
    pub polls: usize,
    /// Number of successful acknowledgements.
    pub acks: usize,
}

impl ProbePort {
    pub(crate) fn success() -> Self {
        let queue = QueueId::new(0);
        Self {
            state: Arc::new(Mutex::new(State {
                completions: [
                    CompletionRecord::new(queue, CommandId::new(0), CompletionStatus::Success),
                    CompletionRecord::new(queue, CommandId::new(1), CompletionStatus::Success),
                ]
                .into(),
                events: [EventRecord::new(
                    queue,
                    CommandId::new(2),
                    EventErrorKind::UnknownBuffer,
                    BufferId::new(99),
                )]
                .into(),
                polls: 0,
                acks: 0,
                fail_poll: false,
                fail_ack: false,
            })),
        }
    }

    pub(crate) fn with_failures(fail_poll: bool, fail_ack: bool) -> Self {
        let port = Self::success();
        let mut state = lock(&port.state);
        state.fail_poll = fail_poll;
        state.fail_ack = fail_ack;
        drop(state);
        port
    }

    pub(crate) fn snapshot(&self) -> PortSnapshot {
        let state = lock(&self.state);
        PortSnapshot {
            polls: state.polls,
            acks: state.acks,
        }
    }
}

impl DriverPort for ProbePort {
    async fn create_queue(&mut self, config: QueueConfig) -> Result<QueueConfigHandle, PortError> {
        Ok(QueueConfigHandle::new(
            DeviceEpoch::initial(),
            QueueDirectoryEntry::new(QueueId::new(0), CompletionRingId::new(0), config),
        ))
    }

    async fn allocate_buffer(&mut self, _len: usize) -> Result<BufferId, PortError> {
        Ok(BufferId::new(0))
    }

    async fn write_buffer(
        &mut self,
        _id: BufferId,
        _offset: usize,
        _data: &[u8],
    ) -> Result<(), PortError> {
        Ok(())
    }

    async fn read_buffer(
        &self,
        _id: BufferId,
        _offset: usize,
        len: usize,
    ) -> Result<Vec<u8>, PortError> {
        Ok(vec![0; len])
    }

    async fn submit_command(
        &mut self,
        _queue: QueueConfigHandle,
        _command: ExternalCommand,
    ) -> Result<(), PortError> {
        Ok(())
    }

    async fn ring_doorbell(&mut self, _queue: QueueConfigHandle) -> Result<(), PortError> {
        Ok(())
    }

    async fn poll_completion(
        &mut self,
        _queue: QueueConfigHandle,
    ) -> Result<Option<CompletionRecord>, PortError> {
        let mut state = lock(&self.state);
        state.polls = state.polls.saturating_add(1);
        if state.fail_poll {
            return Err(PortError::UnknownCompletionRing(CompletionRingId::new(0)));
        }
        Ok(state.completions.pop_front())
    }

    async fn poll_queue_event(
        &mut self,
        _queue: QueueConfigHandle,
    ) -> Result<Option<EventRecord>, PortError> {
        let mut state = lock(&self.state);
        state.polls = state.polls.saturating_add(1);
        if state.fail_poll {
            return Err(PortError::UnknownQueue(QueueId::new(0)));
        }
        Ok(state.events.pop_front())
    }

    async fn configure_irq_route(
        &mut self,
        _source: EpochIrqSource,
        _route: IrqRoute,
    ) -> Result<(), PortError> {
        Ok(())
    }
    async fn enable_irq_source(&mut self, _source: EpochIrqSource) -> Result<(), PortError> {
        Ok(())
    }
    async fn disable_irq_source(&mut self, _source: EpochIrqSource) -> Result<(), PortError> {
        Ok(())
    }
    async fn mask_irq_source(&mut self, _source: EpochIrqSource) -> Result<(), PortError> {
        Ok(())
    }
    async fn unmask_irq_source(&mut self, _source: EpochIrqSource) -> Result<(), PortError> {
        Ok(())
    }

    async fn ack_irq_source(&mut self, source: EpochIrqSource) -> Result<(), PortError> {
        let mut state = lock(&self.state);
        if state.fail_ack {
            return Err(PortError::UnknownIrqSource(source.source()));
        }
        state.acks = state.acks.saturating_add(1);
        drop(state);
        Ok(())
    }

    async fn irq_snapshot(&self) -> Option<IrqSnapshot> {
        None
    }
    async fn wait_irq(&self) -> Option<IrqSnapshot> {
        None
    }
    async fn wait_irq_notification(
        &mut self,
    ) -> Option<pdma_model_contract::semantic::IrqNotification> {
        None
    }
}
