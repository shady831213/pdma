use super::backend::{BackendStepStatus, RunnerBackend};
use super::{AsyncRunSummary, AsyncStepStatus};
use pdma_model_contract::ResourceConfig;
use pdma_model_contract::completion::CompletionRecord;
use pdma_model_contract::desc::ExternalCommand;
use pdma_model_contract::error::{ModelError, ModelResult};
use pdma_model_contract::event::EventRecord;
use pdma_model_contract::ids::{BufferId, CompletionRingId, QueueId};
use pdma_model_contract::queue::{QueueConfig, QueueConfigHandle};
use pdma_model_contract::trace::TraceEvent;

/// Synchronous device surface for deterministic bounded steps without a worker task.
#[derive(Debug)]
pub struct SyncDevice {
    backend: RunnerBackend,
    steps: usize,
    last_status: AsyncStepStatus,
}

impl SyncDevice {
    /// Creates a synchronous device with default resource credits.
    pub fn new() -> Self {
        Self::with_resource_config(ResourceConfig::default())
    }

    /// Creates a synchronous device with explicit resource credits.
    pub fn with_resource_config(config: ResourceConfig) -> Self {
        Self {
            backend: RunnerBackend::native_with_resource_config(config),
            steps: 0,
            last_status: AsyncStepStatus::Idle,
        }
    }

    /// Provisions one command queue and one bound completion ring.
    pub fn create_queue(&mut self, config: QueueConfig) -> ModelResult<QueueConfigHandle> {
        self.backend.create_queue(config)
    }

    /// Allocates a device-visible memory buffer.
    pub fn allocate_buffer(&mut self, len: usize) -> BufferId {
        self.backend.allocate_buffer(len)
    }

    /// Writes bytes into a checked device buffer range.
    pub fn write_buffer(&mut self, id: BufferId, offset: usize, data: &[u8]) -> ModelResult<()> {
        self.backend.write_buffer(id, offset, data)
    }

    /// Reads bytes from a checked device buffer range.
    pub fn read_buffer(&self, id: BufferId, offset: usize, len: usize) -> ModelResult<Vec<u8>> {
        self.backend.read_buffer(id, offset, len)
    }

    /// Pushes one command into a queue's command ring.
    pub fn submit_command(
        &mut self,
        queue_id: QueueId,
        command: ExternalCommand,
    ) -> ModelResult<()> {
        self.backend.submit_command(queue_id, command)
    }

    /// Captures a software doorbell for one queue.
    pub fn ring_doorbell(&mut self, queue_id: QueueId) -> ModelResult<()> {
        self.backend.ring_doorbell(queue_id)
    }

    /// Pops one completion from a completion ring.
    pub fn poll_completion(
        &mut self,
        completion_ring_id: CompletionRingId,
    ) -> ModelResult<Option<CompletionRecord>> {
        self.backend.poll_completion(completion_ring_id)
    }

    /// Pops one event for a queue-local event path.
    pub fn poll_queue_event(&mut self, queue_id: QueueId) -> ModelResult<Option<EventRecord>> {
        self.backend.poll_queue_event(queue_id)
    }

    /// Runs deterministic synchronous steps until idle or until the step budget expires.
    pub fn run_until_idle(&mut self, max_steps: usize) -> ModelResult<AsyncRunSummary> {
        let start_steps = self.steps;
        while self.steps - start_steps < max_steps {
            match self.backend.step()? {
                BackendStepStatus::Progressed => {
                    self.steps = self.steps.saturating_add(1);
                    self.last_status = AsyncStepStatus::Progressed;
                }
                BackendStepStatus::Idle => {
                    self.last_status = AsyncStepStatus::Idle;
                    return self.idle_or_budget_exhausted(max_steps, start_steps);
                }
                BackendStepStatus::Stalled(reason) => {
                    self.last_status = AsyncStepStatus::Stalled(reason);
                    return self.idle_or_budget_exhausted(max_steps, start_steps);
                }
            }
        }
        self.idle_or_budget_exhausted(max_steps, start_steps)
    }

    /// Runs deterministic synchronous steps until idle, stalled, or budget exhaustion.
    pub fn run_until_blocked(&mut self, max_steps: usize) -> ModelResult<AsyncRunSummary> {
        let start_steps = self.steps;
        let mut stalls = 0;
        while self.steps - start_steps < max_steps {
            match self.backend.step()? {
                BackendStepStatus::Progressed => {
                    self.steps = self.steps.saturating_add(1);
                    self.last_status = AsyncStepStatus::Progressed;
                }
                BackendStepStatus::Idle => {
                    self.last_status = AsyncStepStatus::Idle;
                    return Ok(AsyncRunSummary {
                        steps: self.steps - start_steps,
                        errors: 0,
                        stalls,
                        last_status: self.last_status,
                    });
                }
                BackendStepStatus::Stalled(reason) => {
                    stalls += 1;
                    self.last_status = AsyncStepStatus::Stalled(reason);
                    return Ok(AsyncRunSummary {
                        steps: self.steps - start_steps,
                        errors: 0,
                        stalls,
                        last_status: self.last_status,
                    });
                }
            }
        }
        if self.backend.has_work() {
            Err(ModelError::StepBudgetExhausted { max_steps })
        } else {
            Ok(AsyncRunSummary {
                steps: self.steps - start_steps,
                errors: 0,
                stalls,
                last_status: AsyncStepStatus::Idle,
            })
        }
    }

    /// Enables deterministic trace capture.
    pub const fn enable_tracing(&mut self) {
        self.backend.enable_tracing();
    }

    /// Disables trace capture without clearing existing events.
    pub const fn disable_tracing(&mut self) {
        self.backend.disable_tracing();
    }

    /// Clears captured trace events and sequence ordinals.
    pub fn clear_trace(&mut self) {
        self.backend.clear_trace();
    }

    /// Returns captured trace events in deterministic sequence order.
    pub fn trace_events(&self) -> Vec<TraceEvent> {
        self.backend.trace_events()
    }

    /// Exports captured trace events as Chrome/Perfetto JSON.
    pub fn trace_json(&self) -> String {
        self.backend.trace_json()
    }

    fn idle_or_budget_exhausted(
        &self,
        max_steps: usize,
        start_steps: usize,
    ) -> ModelResult<AsyncRunSummary> {
        if self.backend.has_work() {
            Err(ModelError::StepBudgetExhausted { max_steps })
        } else {
            Ok(AsyncRunSummary {
                steps: self.steps - start_steps,
                errors: 0,
                stalls: 0,
                last_status: self.last_status,
            })
        }
    }
}

impl Default for SyncDevice {
    fn default() -> Self {
        Self::new()
    }
}
