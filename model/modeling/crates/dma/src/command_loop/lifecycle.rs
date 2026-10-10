use super::Frame;
use crate::{ChannelTask, Dma, ResetControl};
use hardware_interface_pipeline::{Stage, StepOutcome};
use hardware_interface_runtime::MemoryAccess;
use pdma_model_contract::dma::{ContextId, FaultStatusValues};
use std::convert::Infallible;

pub(super) fn aborts<M, SB>(
    device: &Dma<M, SB>,
    task: &ChannelTask<[Option<ContextId>; 3]>,
) -> bool {
    device.maintenance.as_ref().is_some_and(|maintenance| {
        maintenance.abort
            && maintenance.result == Ok(false)
            && maintenance
                .frontiers
                .iter()
                .any(|frontier| frontier.channel == task.channel)
            && task.value.contains(&Some(maintenance.context))
    })
}

pub(super) fn cleanup<T, M: MemoryAccess<SB>, SB>(frame: &mut Frame<'_, T, M, SB>) -> bool {
    for (index, operation) in frame.parked.iter_mut().enumerate() {
        let discard = frame.device.channels.get(index).is_some_and(|channel| {
            channel
                .parked
                .as_ref()
                .is_some_and(|task| channel.resetting() || aborts(frame.device, task))
        });
        if discard {
            *operation = None;
            if let Some(channel) = frame.device.channels.get_mut(index) {
                channel.parked = None;
                channel.registers.values.fault_status = FaultStatusValues::new(0);
            }
        }
    }
    frame.device.discard_aborted()
}

impl<T, M: MemoryAccess<SB>, SB> Stage<Frame<'_, T, M, SB>> for ResetControl {
    type Input = ();
    type Output = Infallible;
    type Error = Infallible;
    type Wait = Infallible;
    type State = ();

    fn start((): ()) {}

    fn step(
        &mut self,
        frame: &mut Frame<'_, T, M, SB>,
        (): (),
    ) -> StepOutcome<(), Infallible, Infallible, Infallible> {
        frame.device.refresh_maintenance();
        self.step(frame.device, ())
    }
}
