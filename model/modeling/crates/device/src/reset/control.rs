use crate::Device;
use hardware_interface_pipeline::{Stage, StepOutcome};
use pdma_model_contract::{ChannelId, DeviceControlValues};
use std::convert::Infallible;

/// Completes requested resets only after accepted work has retired.
#[derive(Debug, Default)]
pub struct ResetControl;

impl<M> Stage<Device<M>> for ResetControl {
    type Input = ();
    type Output = Infallible;
    type Error = Infallible;
    type Wait = Infallible;
    type State = ();

    fn start((): ()) {}

    fn step(
        &mut self,
        device: &mut Device<M>,
        (): &mut (),
    ) -> StepOutcome<Infallible, Infallible, Infallible> {
        if device.resetting() {
            if device.active.is_none() {
                for channel in &mut device.channels {
                    channel.finish_reset();
                }
                device.control.values = DeviceControlValues {
                    channel_num: device.control.values.channel_num,
                    ..DeviceControlValues::default()
                };
                device.interrupt = false;
            }
        } else {
            for (index, channel) in device.channels.iter_mut().enumerate() {
                if channel.resetting()
                    && device.active.map(|(channel, _)| channel) != Some(ChannelId::new(index))
                {
                    channel.finish_reset();
                }
            }
        }
        StepOutcome::Progressed
    }
}
