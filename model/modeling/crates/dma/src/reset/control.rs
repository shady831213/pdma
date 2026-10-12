use crate::Dma;
use hardware_interface_pipeline::{Stage, StepOutcome, StepResult};
use pdma_model_contract::dma::{ChannelId, DmaControlValues};
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
                    .all(|channel| channel.parked.is_none())
            {
                for channel in &mut device.channels {
                    channel.finish_reset();
                }
                device.control.values = DmaControlValues::default();
                device.interrupt = false;
            }
        } else {
            for (index, channel) in device.channels.iter_mut().enumerate() {
                if channel.resetting()
                    && channel.parked.is_none()
                    && device.active.as_ref().map(super::ChannelTask::channel)
                        != Some(ChannelId::new(index))
                {
                    channel.finish_reset();
                }
            }
        }
        Ok(StepOutcome::Complete(Ok(())))
    }
}
