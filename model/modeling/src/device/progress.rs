use crate::ModelError;
use crate::ModelResult;
use crate::device::PdmaDevice;
use crate::resources::ResourceDemand;
use crate::scheduler::Scheduler;

/// Summary returned by `run_until_idle`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RunSummary {
    /// Number of device steps that changed state.
    pub steps: usize,
}

/// Deterministic outcome of one status-annotated device step.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepStatus {
    /// The step retired work or published a pending retirement.
    Progressed,
    /// The device has no runnable or pending work.
    Idle,
    /// The device has work but cannot make progress until software relieves pressure.
    Stalled(StallReason),
}

/// Resource or retirement pressure that prevented a status-annotated step.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StallReason {
    /// Execution resources were below the selected command's demand.
    ExecutionResourcePressure,
    /// A completion record could not be published.
    CompletionPublicationPressure,
    /// A newly faulting command could not publish its event record.
    EventPublicationPressure,
    /// A previously blocked fault event still cannot be published on retry.
    FaultRetirementPendingRetry,
}

/// Summary returned by `run_until_blocked`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RunStatusSummary {
    /// Number of status-annotated steps that changed state.
    pub steps: usize,
    /// Terminal idle or stall status observed after the progressed steps.
    pub terminal_status: StepStatus,
}

impl PdmaDevice {
    /// Advances the deterministic device model by at most one command and reports stall reasons.
    ///
    /// # Errors
    /// Returns the original model error when command execution faults after publishing its event.
    pub fn step_with_status(&mut self) -> ModelResult<StepStatus> {
        if let Some(pending) = self.pending_retirement {
            return self.retry_pending_retirement_with_status(pending);
        }
        let Some(queue_id) = self.arbiter.select_next(&mut self.runnable) else {
            self.trace.idle();
            return Ok(StepStatus::Idle);
        };
        let Some(command) = self.queue(queue_id)?.command_ring.front() else {
            return Ok(StepStatus::Progressed);
        };
        let work = Scheduler::lower(command.clone())?;
        self.trace.scheduled(&work);
        let demand = ResourceDemand::for_work(&work);
        if !self.resources.has_available(demand) {
            self.runnable.push(queue_id);
            self.trace
                .stall(Some(queue_id), "ExecutionResourcePressure");
            return Ok(StepStatus::Stalled(StallReason::ExecutionResourcePressure));
        }
        self.resources.acquire(demand);
        let result = self.retire_ready_work_with_status(queue_id, &work);
        self.resources.release(demand);
        result
    }

    /// Advances the deterministic device model by at most one command.
    pub fn step(&mut self) -> ModelResult<bool> {
        Ok(self.step_with_status()? == StepStatus::Progressed)
    }

    /// Runs deterministic steps until idle or until the step budget expires.
    pub fn run_until_idle(&mut self, max_steps: usize) -> ModelResult<RunSummary> {
        let mut steps = 0;
        while steps < max_steps {
            if !self.step()? {
                return if self.has_work() {
                    Err(ModelError::StepBudgetExhausted { max_steps })
                } else {
                    Ok(RunSummary { steps })
                };
            }
            steps += 1;
        }
        if self.has_work() {
            Err(ModelError::StepBudgetExhausted { max_steps })
        } else {
            Ok(RunSummary { steps })
        }
    }

    /// Runs deterministic steps until the device becomes idle, stalls, or the budget expires.
    ///
    /// # Errors
    /// Returns the original model error when a fault event publishes, or
    /// `ModelError::StepBudgetExhausted` when the budget is consumed while work remains runnable.
    pub fn run_until_blocked(&mut self, max_steps: usize) -> ModelResult<RunStatusSummary> {
        let mut steps = 0;
        while steps < max_steps {
            match self.step_with_status()? {
                StepStatus::Progressed => steps += 1,
                StepStatus::Idle => {
                    return Ok(RunStatusSummary {
                        steps,
                        terminal_status: StepStatus::Idle,
                    });
                }
                StepStatus::Stalled(reason) => {
                    return Ok(RunStatusSummary {
                        steps,
                        terminal_status: StepStatus::Stalled(reason),
                    });
                }
            }
        }
        if self.has_work() {
            Err(ModelError::StepBudgetExhausted { max_steps })
        } else {
            Ok(RunStatusSummary {
                steps,
                terminal_status: StepStatus::Idle,
            })
        }
    }

    fn has_work(&self) -> bool {
        self.pending_retirement.is_some()
            || !self.runnable.is_empty()
            || self
                .queues
                .iter()
                .any(|queue| !queue.command_ring.is_empty())
    }
}
