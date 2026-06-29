use crate::ModelError;
use crate::ModelResult;
use crate::completion::{CompletionRecord, CompletionStatus};
use crate::desc::ScheduledWork;
use crate::device::PdmaDevice;
use crate::engines::{CopyEngine, EngineError, VectorAddEngine};
use crate::event::EventRecord;
use crate::ids::{CompletionRingId, QueueId};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum PendingRetirement {
    Completion {
        queue_id: QueueId,
        completion_ring_id: CompletionRingId,
        record: CompletionRecord,
    },
    Event {
        queue_id: QueueId,
        record: EventRecord,
        source: ModelError,
    },
}

impl PendingRetirement {
    pub(super) const fn completion(
        queue_id: QueueId,
        completion_ring_id: CompletionRingId,
        record: CompletionRecord,
    ) -> Self {
        Self::Completion {
            queue_id,
            completion_ring_id,
            record,
        }
    }

    pub(super) const fn event(queue_id: QueueId, record: EventRecord, source: ModelError) -> Self {
        Self::Event {
            queue_id,
            record,
            source,
        }
    }

    pub(super) const fn queue_id(self) -> QueueId {
        match self {
            Self::Completion { queue_id, .. } | Self::Event { queue_id, .. } => queue_id,
        }
    }
}

impl PdmaDevice {
    pub(super) fn retire_ready_work(
        &mut self,
        queue_id: QueueId,
        work: &ScheduledWork,
    ) -> ModelResult<bool> {
        let _retired = self.queue_mut(queue_id)?.command_ring.pop();
        let has_more_commands = !self.queue(queue_id)?.command_ring.is_empty();
        let result = self.build_retirement(queue_id, work);
        if has_more_commands && self.pending_retirement.is_none() {
            self.runnable.push(queue_id);
        }
        result
    }

    pub(super) fn retry_pending_retirement(
        &mut self,
        pending: PendingRetirement,
    ) -> ModelResult<bool> {
        let result = self.publish_retirement(pending);
        if self.pending_retirement.is_none()
            && !self.queue(pending.queue_id())?.command_ring.is_empty()
        {
            self.runnable.push(pending.queue_id());
        }
        result
    }

    fn build_retirement(&mut self, queue_id: QueueId, work: &ScheduledWork) -> ModelResult<bool> {
        if let Err(error) = self.execute_work(work) {
            return match error {
                EngineError::Memory(fault) => {
                    let event = EventRecord::new(
                        work.queue_id(),
                        work.command_id(),
                        fault.error_kind(),
                        fault.buffer(),
                    );
                    self.publish_retirement(PendingRetirement::event(
                        queue_id,
                        event,
                        fault.source(),
                    ))
                }
                EngineError::Model(error) => Err(error),
            };
        }
        let completion = CompletionRecord::new(
            work.queue_id(),
            work.command_id(),
            CompletionStatus::Success,
        );
        self.publish_retirement(PendingRetirement::completion(
            queue_id,
            work.completion_ring_id(),
            completion,
        ))
    }

    fn publish_retirement(&mut self, pending: PendingRetirement) -> ModelResult<bool> {
        match pending {
            PendingRetirement::Completion {
                queue_id: _,
                completion_ring_id,
                record,
            } => self.publish_completion(pending, completion_ring_id, record),
            PendingRetirement::Event {
                queue_id: _,
                record,
                source,
            } => self.publish_event(pending, record, source),
        }
    }

    fn publish_completion(
        &mut self,
        pending: PendingRetirement,
        completion_ring_id: CompletionRingId,
        record: CompletionRecord,
    ) -> ModelResult<bool> {
        let (completion_ring_full, occupied_completions) = {
            let completion_ring = self.completion_mut(completion_ring_id)?;
            (completion_ring.is_full(), completion_ring.len())
        };
        if completion_ring_full
            || !self
                .resources
                .has_completion_publication_slot(occupied_completions)
        {
            self.pending_retirement = Some(pending);
            return Ok(false);
        }
        self.completion_mut(completion_ring_id)?.push(record)?;
        self.pending_retirement = None;
        Ok(true)
    }

    fn publish_event(
        &mut self,
        pending: PendingRetirement,
        record: EventRecord,
        source: ModelError,
    ) -> ModelResult<bool> {
        if !self
            .resources
            .has_event_publication_slot(self.event_ring.len())
        {
            self.pending_retirement = Some(pending);
            return Ok(false);
        }
        self.event_ring.push_back(record);
        self.pending_retirement = None;
        Err(source)
    }

    fn execute_work(&mut self, work: &ScheduledWork) -> Result<(), EngineError> {
        match work {
            ScheduledWork::Copy(copy) => CopyEngine::execute(&mut self.memory, copy),
            ScheduledWork::VectorAdd(vector_add) => {
                VectorAddEngine::execute(&mut self.memory, *vector_add)
            }
        }
    }
}
