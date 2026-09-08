use std::convert::Infallible;

use hardware_interface_pipeline::{Stage, StepOutcome};

use super::context::HardwareContext;
use crate::irq::PdmaIrqController;

impl Stage<HardwareContext<'_>> for PdmaIrqController {
    type Input = ();
    type Output = Infallible;
    type Error = Infallible;
    type Wait = Infallible;
    type State = ();

    fn start((): Self::Input) -> Self::State {}

    fn step(
        &mut self,
        context: &mut HardwareContext<'_>,
        (): &mut Self::State,
    ) -> StepOutcome<Self::Output, Self::Error, Self::Wait> {
        self.update(context.queues, context.trace);
        StepOutcome::Progressed
    }
}
