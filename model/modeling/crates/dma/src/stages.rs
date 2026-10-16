mod candidate;
mod data_path;
mod execution;
mod fetch;
mod publish;
#[cfg(test)]
mod tests;

use crate::{ChunkInput, Dma, Error, ExecutionContext, Responses, RoundRobin};
pub use candidate::{Candidate, CommandRequest};
pub use data_path::{Grant, Hardware};
pub use execution::{CheckRanges, CompletionInput, PrepareChunk, ReadCopy, WriteCopy};
pub use fetch::Fetch;
use hardware_interface_pipeline::{Arbitrate, FanIn, Stage, StepOutcome, StepResult, Then};
use pdma_model_contract::dma::ChannelId;
pub use publish::Publish;

/// One concrete chunk transferred through preparation, read and write.
pub type CopyChunk = Then<PrepareChunk, Then<ReadCopy, WriteCopy>>;
/// Execution of one bounded chunk of already validated hardware traversal.
pub type Execution = CopyChunk;
/// Shared execution followed by response dispatch.
pub type Pipeline = Then<ReadPipeline, WritePipeline>;
/// Preparation and payload read before optional custom transformation.
pub type ReadPipeline = Then<PrepareChunk, ReadCopy>;
/// Payload write and response publication, shared by fresh and resumed writes.
pub type WritePipeline = Then<WriteCopy, Then<CompletionInput, Responses>>;
type Commands = FanIn<Candidate, Arbitrate<RoundRobin>, SelectedRequest>;

#[derive(Debug)]
pub struct SelectedRequest;
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
    /// Wires one finite opaque execution graph over selected channel resources.
    pub fn hardware_with<S, T>(&self, pipeline: S) -> Hardware<S>
    where
        for<'a> S: Stage<
                ExecutionContext<'a, M, SB>,
                Input = Option<ChunkInput>,
                Output = (),
                Error = Error,
                State = T,
            >,
    {
        Hardware::new(
            FanIn::new(
                (0..self.channels.len())
                    .map(|index| Candidate::new(ChannelId::new(index)))
                    .collect(),
                Arbitrate::snapshots(RoundRobin::default()),
                SelectedRequest,
            ),
            pipeline,
            self.channels.len(),
        )
    }
}
impl<M: hardware_interface_runtime::MemoryAccess<SB>, SB> Dma<M, SB> {
    /// Builds the finite connected command admission and bounded execution graph.
    pub fn hardware(&self) -> Hardware
    where
        M::Error: Into<crate::MemoryFailure>,
    {
        self.hardware_with(Then::new(
            Then::new(PrepareChunk, ReadCopy),
            Then::new(WriteCopy, Then::new(CompletionInput, self.responses())),
        ))
    }
}
