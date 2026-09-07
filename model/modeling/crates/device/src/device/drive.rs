use hardware_interface_pipeline::StepOutcome;

use super::DeviceCore;
use super::probe::{RunStateProbe, probe_run_state};
#[cfg(test)]
use crate::ModelResult;
use crate::status::AsyncStallReason;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ModelStepStatus {
    Progressed,
    FaultCached,
    Idle,
    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "test-only stepped compatibility")
    )]
    Stalled(AsyncStallReason),
}

pub(crate) type CommandStepOutcome =
    StepOutcome<ModelStepStatus, crate::ModelError, AsyncStallReason>;

enum Admission {
    Accepted,
    Idle,
    Starved,
}

impl DeviceCore {
    pub(crate) fn step_pipeline(&mut self) -> CommandStepOutcome {
        if self.command_pipeline.is_empty() {
            match self.scan_admission() {
                Admission::Accepted => {}
                Admission::Idle => {
                    self.trace.idle();
                    return StepOutcome::Complete(Ok(ModelStepStatus::Idle));
                }
                Admission::Starved => {
                    return StepOutcome::Pending(AsyncStallReason::ExecutionResourcePressure);
                }
            }
        }
        let mut slot = std::mem::replace(
            &mut self.command_pipeline,
            super::command_pipeline::PipelineSlot::new(),
        );
        let outcome = slot.step(self);
        self.command_pipeline = slot;
        outcome
    }

    pub(crate) fn clear_command_pipeline(&mut self) {
        self.command_pipeline.clear();
    }

    fn scan_admission(&mut self) -> Admission {
        let candidates = self.scheduler.runnable_len();
        if candidates == 0 {
            return match probe_run_state(
                &self.queues,
                &self.dma,
                &self.scheduler,
                &self.execution,
                &self.retirement,
            ) {
                RunStateProbe::Idle => Admission::Idle,
                RunStateProbe::Running | RunStateProbe::Blocked { .. } => Admission::Accepted,
            };
        }
        let mut first_starved = None;
        for _ in 0..candidates {
            match probe_run_state(
                &self.queues,
                &self.dma,
                &self.scheduler,
                &self.execution,
                &self.retirement,
            ) {
                RunStateProbe::Blocked {
                    queue,
                    reason: AsyncStallReason::ExecutionResourcePressure,
                } => {
                    first_starved.get_or_insert(queue);
                    self.scheduler.rotate_current();
                }
                RunStateProbe::Idle => return Admission::Idle,
                RunStateProbe::Running | RunStateProbe::Blocked { .. } => {
                    return Admission::Accepted;
                }
            }
        }
        match first_starved {
            Some(queue) => {
                self.trace.stall(Some(queue), "ExecutionResourcePressure");
                Admission::Starved
            }
            None => Admission::Accepted,
        }
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
