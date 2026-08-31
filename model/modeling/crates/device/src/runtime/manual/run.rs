use super::SyncDevice;
use crate::engine::step::ModelStepStatus;
use crate::runtime::{AsyncRunSummary, AsyncStepStatus};
use crate::{ModelError, ModelResult};

impl SyncDevice {
    /// Runs until idle and preserves the execution-layer source chain.
    pub fn run_until_idle_typed(
        &mut self,
        max_steps: usize,
    ) -> Result<AsyncRunSummary, crate::PdmaRunError> {
        self.run_until_idle(max_steps)
            .map_err(crate::PdmaRunError::from)
    }

    /// Runs until blocked and preserves the execution-layer source chain.
    pub fn run_until_blocked_typed(
        &mut self,
        max_steps: usize,
    ) -> Result<AsyncRunSummary, crate::PdmaRunError> {
        self.run_until_blocked(max_steps)
            .map_err(crate::PdmaRunError::from)
    }

    /// Runs deterministic synchronous steps until idle or until the step budget expires.
    pub fn run_until_idle(&mut self, max_steps: usize) -> ModelResult<AsyncRunSummary> {
        let start_steps = self.steps;
        while self.steps - start_steps < max_steps {
            match self.step_once()? {
                ModelStepStatus::Progressed => {
                    self.steps = self.steps.saturating_add(1);
                    self.last_status = AsyncStepStatus::Progressed;
                }
                ModelStepStatus::FaultCached => {
                    self.last_status = AsyncStepStatus::Progressed;
                }
                ModelStepStatus::Idle => {
                    self.last_status = AsyncStepStatus::Idle;
                    return self.idle_or_budget_exhausted(max_steps, start_steps);
                }
                ModelStepStatus::Stalled(reason) => {
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
            match self.step_once()? {
                ModelStepStatus::Progressed => {
                    self.steps = self.steps.saturating_add(1);
                    self.last_status = AsyncStepStatus::Progressed;
                }
                ModelStepStatus::FaultCached => {
                    self.last_status = AsyncStepStatus::Progressed;
                }
                ModelStepStatus::Idle => {
                    self.last_status = AsyncStepStatus::Idle;
                    return Ok(self.summary(start_steps, stalls));
                }
                ModelStepStatus::Stalled(reason) => {
                    stalls += 1;
                    self.last_status = AsyncStepStatus::Stalled(reason);
                    return Ok(self.summary(start_steps, stalls));
                }
            }
        }
        if self.has_work() {
            Err(ModelError::StepBudgetExhausted { max_steps })
        } else {
            self.last_status = AsyncStepStatus::Idle;
            Ok(self.summary(start_steps, stalls))
        }
    }

    fn step_once(&mut self) -> ModelResult<ModelStepStatus> {
        self.core.device.step()
    }

    fn idle_or_budget_exhausted(
        &self,
        max_steps: usize,
        start_steps: usize,
    ) -> ModelResult<AsyncRunSummary> {
        if self.has_work() {
            Err(ModelError::StepBudgetExhausted { max_steps })
        } else {
            Ok(self.summary(start_steps, 0))
        }
    }

    fn has_work(&self) -> bool {
        self.device
            .execution
            .has_work(&self.device.dma, &self.device.queue_control)
    }

    const fn summary(&self, start_steps: usize, stalls: usize) -> AsyncRunSummary {
        AsyncRunSummary {
            steps: self.core.steps - start_steps,
            errors: 0,
            stalls,
            last_status: self.core.last_status,
        }
    }
}
