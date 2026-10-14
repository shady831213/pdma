use crate::Dma;
use hardware_interface_pipeline::{Stage, StepOutcome, StepResult};
use pdma_model_contract::dma::DmaControlValues;
use std::convert::Infallible;

/// Completes requested resets only after accepted work has retired.
#[derive(Debug, Default)]
pub struct ResetControl;

impl<M, SB> Stage<Dma<M, SB>> for ResetControl {
    type Input = ();
    type Output = ();
    type Error = Infallible;
    type State = ();

    fn start((): ()) {}

    fn step(&mut self, device: &mut Dma<M, SB>, (): &mut ()) -> StepResult<(), Infallible> {
        if device.resetting() {
            if device.active.is_none()
                && device
                    .channels
                    .iter()
                    .all(|channel| channel.accepted_command().is_none())
            {
                for channel in &mut device.channels {
                    channel.finish_reset();
                }
                device.control.values = DmaControlValues::default();
                device.interrupt = false;
            }
        } else {
            for channel in &mut device.channels {
                if channel.resetting() && channel.accepted_command().is_none() {
                    channel.finish_reset();
                }
            }
        }
        Ok(StepOutcome::Complete(Ok(())))
    }
}
