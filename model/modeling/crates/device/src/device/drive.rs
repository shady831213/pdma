use hardware_interface_pipeline::{StepOutcome, Then};

use super::DeviceCore;
use super::context::HardwareContext;
#[cfg(test)]
use crate::ModelResult;
use crate::status::AsyncStallReason;

/// Observable completion of one singleton hardware service request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HardwareStepStatus {
    /// A command completed execution and retirement.
    Progressed,
    /// A frontend fault was cached and awaits a separate retirement attempt.
    FaultCached,
    /// The device has no runnable work.
    Idle,
    /// The device requires an external capacity or queue-state change.
    Stalled(AsyncStallReason),
}

pub(crate) use HardwareStepStatus as ModelStepStatus;

pub(crate) type CommandStepOutcome =
    StepOutcome<ModelStepStatus, crate::ModelError, AsyncStallReason>;

impl DeviceCore {
    pub(crate) fn step_pipeline(&mut self) -> CommandStepOutcome {
        let mut context = HardwareContext {
            queues: &mut self.queues,
            runnable: &mut self.runnable,
            memory: &self.memory,
            irq: &mut self.irq,
            trace: &mut self.trace,
            fetches: &mut self.native_fetches,
            resources: self.execution.resources(),
        };
        let mut stages = Then::new(
            &mut self.issue,
            Then::new(&mut self.execution, &mut self.retirement),
        );
        let outcome = self.command_pipeline.step(&mut context, &mut stages);
        if matches!(outcome, StepOutcome::Complete(_) | StepOutcome::Exhausted) {
            self.command_pipeline.clear();
        }
        outcome
    }

    pub(crate) fn clear_command_pipeline(&mut self) {
        self.command_pipeline.clear();
    }

    #[cfg(test)]
    pub(crate) fn step(&mut self) -> ModelResult<ModelStepStatus> {
        loop {
            match self.step_pipeline() {
                StepOutcome::Progressed => {}
                StepOutcome::Pending(reason) => return Ok(ModelStepStatus::Stalled(reason)),
                StepOutcome::Complete(result) => return result,
                StepOutcome::Exhausted => {
                    panic!("command pipeline exhausted before terminal result")
                }
            }
        }
    }
}
