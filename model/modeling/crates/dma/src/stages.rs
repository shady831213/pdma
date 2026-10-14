mod candidate;
mod execution;
mod fetch;
mod publish;

pub use candidate::{Candidate, CommandRequest};
pub use execution::{CheckRanges, NextChunk, ReadCopy, WriteCopy};
pub use fetch::Fetch;
pub use publish::Publish;

use crate::{Dma, Error, ResetControl, Responses, RoundRobin, Work};
use hardware_interface_pipeline::{
    Arbitrate, FanIn, RunError, Stage, StepOutcome, StepResult, Then,
};
use hardware_interface_runtime::MemoryAccess;
use pdma_model_contract::dma::ChannelId;
use std::convert::Infallible;

/// Refreshes every channel's visible interrupt causes from hardware state.
#[derive(Debug, Default)]
pub struct UpdateIrq;

impl<M, SB> Stage<Dma<M, SB>> for UpdateIrq {
    type Input = ();
    type Output = ();
    type Error = Infallible;
    type State = ();
    fn start((): ()) {}
    fn step(&mut self, device: &mut Dma<M, SB>, (): &mut ()) -> StepResult<(), Infallible> {
        for channel in &mut device.channels {
            channel.update_irq();
        }
        Ok(StepOutcome::Complete(Ok(())))
    }
}

/// One bounded chunk preparation, read and write composition.
pub type CopyChunk = Then<Then<NextChunk, ReadCopy>, WriteCopy>;
/// Range validation followed by one bounded payload chunk.
pub type Execution = Then<CheckRanges, CopyChunk>;
/// Shared execution followed by response dispatch.
pub type Pipeline = Then<Execution, Responses>;

type Commands<S> = FanIn<Candidate, Arbitrate<RoundRobin>, Then<Fetch, S>>;

/// Finite command arbitration with independently stepped IRQ and reset hardware.
#[derive(Debug)]
pub struct Service<S = Pipeline> {
    command: Commands<S>,
    count: usize,
    reset: ResetControl,
    irq: UpdateIrq,
}

/// Invocation-local fan state; persistent command state belongs to channels.
#[derive(Debug)]
pub struct ServiceState<T> {
    command: Option<T>,
}

impl<M: MemoryAccess<SB>, SB, S> Stage<Dma<M, SB>> for Service<S>
where
    S: Stage<Dma<M, SB>, Input = Option<Work>, Output = (), Error = Error>,
{
    type Input = ();
    type Output = ();
    type Error = Error;
    type State = Option<ServiceState<<Commands<S> as Stage<Dma<M, SB>>>::State>>;
    fn start((): ()) -> Self::State {
        Some(ServiceState { command: None })
    }

    fn step(&mut self, device: &mut Dma<M, SB>, state: &mut Self::State) -> StepResult<(), Error> {
        let retained = state.as_mut().ok_or(RunError::Exhausted)?;
        device.retire_stopped_work();
        let command = retained
            .command
            .get_or_insert_with(|| <Commands<S> as Stage<Dma<M, SB>>>::start(vec![(); self.count]));
        let outcome = if device.discard_aborted() {
            Ok(StepOutcome::Complete(Ok(Vec::new())))
        } else {
            self.command.step(device, command)
        };
        device.retire_stopped_work();
        device.refresh_maintenance();
        let irq = self.irq.step(device, &mut ());
        let reset = self.reset.step(device, &mut ());
        let (StepOutcome::Complete(Ok(())), StepOutcome::Complete(Ok(()))) = (irq?, reset?) else {
            return Err(RunError::Exhausted);
        };
        match outcome {
            Ok(StepOutcome::InProgress) => Ok(StepOutcome::InProgress),
            Ok(StepOutcome::Complete(result)) => {
                *state = None;
                let result = result.map(|_| ()).map_err(Error::from);
                Ok(StepOutcome::Complete(match result {
                    Err(Error::Reset | Error::Halted | Error::ChannelFatal(_)) | Ok(()) => Ok(()),
                    Err(error) => Err(error),
                }))
            }
            Err(error) => {
                *state = None;
                Err(error)
            }
        }
    }
}

impl<M, SB> Dma<M, SB> {
    /// Builds response dispatch ports bound to this instance's channels.
    pub fn responses(&self) -> Responses {
        Responses::new(
            (0..self.channels.len())
                .map(|index| Publish::new(ChannelId::new(index)))
                .collect(),
        )
    }

    /// Wires a custom finite shared graph after descriptor acceptance.
    pub fn service_with<S>(&self, pipeline: S) -> Service<S>
    where
        S: Stage<Self, Input = Option<Work>, Output = (), Error = Error>,
    {
        Service {
            command: FanIn::new(
                (0..self.channels.len())
                    .map(|index| Candidate::new(ChannelId::new(index)))
                    .collect(),
                Arbitrate::snapshots(RoundRobin::default()),
                Then::new(Fetch, pipeline),
            ),
            count: self.channels.len(),
            reset: ResetControl,
            irq: UpdateIrq,
        }
    }
}

impl<M: MemoryAccess<SB>, SB> Dma<M, SB> {
    /// Builds the default finite command service and maintenance wiring.
    pub fn service(&self) -> Service
    where
        M::Error: Into<crate::MemoryFailure>,
    {
        self.service_with(Then::new(
            Then::new(
                CheckRanges,
                Then::new(Then::new(NextChunk, ReadCopy), WriteCopy),
            ),
            self.responses(),
        ))
    }
}
