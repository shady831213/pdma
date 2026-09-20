use crate::{ChannelTask, Dma, Error};
use hardware_interface_pipeline::{ReadyStage, Stage, StepOutcome};
use hardware_interface_runtime::MemoryAccess;
use pdma_model_contract::{ChannelId, Command, Ring};

/// A channel's non-consuming eligibility check and command acceptance endpoint.
#[derive(Debug)]
pub struct Fetch {
    channel: ChannelId,
}

impl Fetch {
    /// Binds an input port to one channel without creating execution state.
    pub const fn new(channel: ChannelId) -> Self {
        Self { channel }
    }
}

impl<M: MemoryAccess<SB>, SB> ReadyStage<Dma<M, SB>> for Fetch {
    fn ready(&self, device: &Dma<M, SB>) -> bool {
        if device.halted() || device.resetting() {
            return false;
        }
        device
            .channels
            .get(self.channel.index())
            .is_none_or(|channel| {
                channel.enabled()
                    && !channel.resetting()
                    && !channel.halted()
                    && !channel.command.pointers().is_empty()
                    && channel.response.pointers().pending() < channel.response.capacity()
            })
    }
}

impl<M: MemoryAccess<SB>, SB> Stage<Dma<M, SB>> for Fetch {
    type Input = ();
    type Output = ChannelTask<Command>;
    type Error = Error;
    type Wait = ();
    type State = ();

    fn start((): ()) {}

    fn step(
        &mut self,
        device: &mut Dma<M, SB>,
        (): &mut (),
    ) -> StepOutcome<Self::Output, Error, ()> {
        if !self.ready(device) {
            return StepOutcome::Pending(());
        }
        let outcome = {
            let selected = match device.channel_context(self.channel) {
                Ok(selected) => selected,
                Err(error) => return StepOutcome::Complete(Err(error)),
            };
            let outcome = match selected.channel.command.pop(&selected.memory) {
                Ok(Some(value)) => StepOutcome::Complete(Ok(ChannelTask {
                    channel: self.channel,
                    generation: selected.channel.generation,
                    value,
                })),
                Ok(None) => StepOutcome::Pending(()),
                Err(fault) => {
                    selected.channel.latch(fault);
                    StepOutcome::Complete(Err(Error::Fault(fault)))
                }
            };
            selected.channel.update_irq();
            outcome
        };
        if let StepOutcome::Complete(Ok(task)) = &outcome {
            device.active = Some((task.channel(), task.generation));
        }
        outcome
    }
}
