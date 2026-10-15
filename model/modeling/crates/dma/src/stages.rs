mod candidate;
mod execution;
mod fetch;
mod publish;
mod service;

use crate::{ChunkInput, Dma, Error, ExecutionContext, ResetControl, Responses, RoundRobin};
pub use candidate::{Candidate, CommandRequest};
pub use execution::{CheckRanges, ExecutionError, PrepareChunk, ReadCopy, WriteCopy};
pub use fetch::Fetch;
use hardware_interface_pipeline::{
    Arbitrate, FanIn, MapError, Stage, StepOutcome, StepResult, Then,
};
use pdma_model_contract::dma::ChannelId;
pub use publish::Publish;
pub use service::Service;
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

/// One concrete chunk transferred through preparation, read and write.
pub type CopyChunk = Then<PrepareChunk, Then<ReadCopy, WriteCopy>>;
/// Execution of one bounded chunk of already validated hardware traversal.
pub type Execution = CopyChunk;
/// Shared execution followed by response dispatch.
pub type Pipeline = Then<ReadPipeline, WritePipeline>;
/// Preparation and payload read before optional custom transformation.
pub type ReadPipeline = Then<PrepareChunk, ReadCopy>;
/// Payload write and response publication, shared by fresh and resumed writes.
pub type WritePipeline = Then<WriteCopy, Responses>;
type Commands = FanIn<Candidate, Arbitrate<RoundRobin>, SelectedRequest>;
type Fresh = Then<MapError<Fetch, fn(Error) -> ExecutionError>, CheckRanges>;

#[derive(Debug)]
struct SelectedRequest;
impl<M, SB> Stage<Dma<M, SB>> for SelectedRequest {
    type Input = Option<CommandRequest>;
    type Output = Option<CommandRequest>;
    type Error = Error;
    type State = Option<Self::Input>;
    fn start(input: Self::Input) -> Self::State {
        Some(input)
    }
    fn step(
        &mut self,
        _: &mut Dma<M, SB>,
        state: &mut Self::State,
    ) -> StepResult<Self::Output, Error> {
        Ok(StepOutcome::Complete(Ok(state.take().ok_or(
            hardware_interface_pipeline::RunError::Exhausted,
        )?)))
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
    /// Wires a finite chunk graph over a selected execution context.
    /// The outer Then splits at Payload: preparation/read/transforms are upstream;
    /// downstream starts with write and is reused directly for write-fault recovery.
    pub fn service_with<A, B, T, R>(&self, pipeline: Then<A, Then<WriteCopy, B>>) -> Service<A, B>
    where
        for<'a> A: Stage<
                ExecutionContext<'a, M, SB>,
                Input = Option<ChunkInput>,
                Output = Option<crate::Payload>,
                Error = ExecutionError,
                State = T,
            >,
        for<'a> B: Stage<
                ExecutionContext<'a, M, SB>,
                Input = Option<crate::Completion>,
                Output = (),
                Error = ExecutionError,
                State = R,
            >,
    {
        Service {
            command: FanIn::new(
                (0..self.channels.len())
                    .map(|index| Candidate::new(ChannelId::new(index)))
                    .collect(),
                Arbitrate::snapshots(RoundRobin::default()),
                SelectedRequest,
            ),
            execution: pipeline,
            fresh: Then::new(
                MapError::new(
                    Fetch,
                    ExecutionError::Rejected as fn(Error) -> ExecutionError,
                ),
                CheckRanges,
            ),
            count: self.channels.len(),
            reset: ResetControl,
            irq: UpdateIrq,
        }
    }
}
impl<M: hardware_interface_runtime::MemoryAccess<SB>, SB> Dma<M, SB> {
    /// Builds admission, bounded chunk execution and independent maintenance wiring.
    pub fn service(&self) -> Service
    where
        M::Error: Into<crate::MemoryFailure>,
    {
        self.service_with(Then::new(
            Then::new(PrepareChunk, ReadCopy),
            Then::new(WriteCopy, self.responses()),
        ))
    }
}
