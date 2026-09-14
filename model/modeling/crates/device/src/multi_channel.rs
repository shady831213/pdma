use crate::{Device, Error, Pipeline};
use hardware_interface_pipeline::{Repeat, Stage, StepOutcome};
use hardware_interface_runtime::MemoryAccess;
use std::ops::ControlFlow;

/// Reuses the sole command pipeline after completion or channel-local recovery.
#[derive(Debug)]
pub struct RunCommand {
    pipeline: Pipeline,
}

/// A sustained shared execution unit, not an array of per-channel operations.
pub type MultiChannel = Repeat<RunCommand>;

impl<M> Device<M> {
    /// Builds the sustained shared command path from this instance's input ports.
    pub fn multi_channel(&self) -> MultiChannel {
        Repeat::new(RunCommand {
            pipeline: self.pipeline(),
        })
    }
}

impl<M: MemoryAccess> Stage<Device<M>> for RunCommand {
    type Input = ();
    type Output = ControlFlow<(), ()>;
    type Error = Error;
    type Wait = ();
    type State = <Pipeline as Stage<Device<M>>>::State;

    fn start((): ()) -> Self::State {
        <Pipeline as Stage<Device<M>>>::start(())
    }

    fn step(
        &mut self,
        device: &mut Device<M>,
        state: &mut Self::State,
    ) -> StepOutcome<Self::Output, Error, ()> {
        match self.pipeline.step(device, state) {
            StepOutcome::Complete(Ok(()) | Err(Error::Reset | Error::Fault(_) | Error::Halted)) => {
                StepOutcome::Complete(Ok(ControlFlow::Continue(())))
            }
            StepOutcome::Complete(Err(error)) => StepOutcome::Complete(Err(error)),
            StepOutcome::Progressed => StepOutcome::Progressed,
            StepOutcome::Pending(()) => StepOutcome::Pending(()),
            StepOutcome::Exhausted => StepOutcome::Exhausted,
        }
    }
}
