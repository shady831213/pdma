mod candidate;
mod data_path;
mod execution;
mod fetch;
mod publish;
#[cfg(test)]
mod tests;

use crate::{Chunk, ChunkInput, Completion, Dma, Error, ExecutionContext, Payload, Responses};
pub use candidate::{Candidate, CommandRequest, RetainedTask};
pub use data_path::Hardware;
pub use execution::{CheckRanges, CompletionInput, PrepareChunk, ReadCopy, WriteCopy};
pub use fetch::Fetch;
use hardware_interface_pipeline::{Stage, Then};
use pdma_model_contract::dma::ChannelId;
pub use publish::Publish;

/// One concrete chunk transferred through preparation, read and write.
pub type CopyChunk = Then<PrepareChunk, Then<ReadCopy, WriteCopy, Option<Payload>>, Option<Chunk>>;
/// Execution of one bounded chunk of already validated hardware traversal.
pub type Execution = CopyChunk;
/// Shared execution followed by response dispatch.
pub type Pipeline = Then<ReadPipeline, WritePipeline, Option<Payload>>;
/// Preparation and payload read before optional custom transformation.
pub type ReadPipeline = Then<PrepareChunk, ReadCopy, Option<Chunk>>;
/// Payload write and response publication, shared by fresh and resumed writes.
pub type WritePipeline =
    Then<WriteCopy, Then<CompletionInput, Responses, Option<Completion>>, Option<Completion>>;
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
    pub fn hardware_with<S>(&self, pipeline: S) -> Hardware<S>
    where
        for<'a> S: Stage<
                ExecutionContext<'a, M, SB>,
                Input = Option<ChunkInput>,
                Output = (),
                Error = Error,
            >,
    {
        Hardware::new(pipeline, self.channels.len())
    }
}
impl<M: hardware_interface_runtime::MemoryAccess<SB>, SB> Dma<M, SB> {
    /// Builds the finite connected command admission and bounded execution graph.
    pub fn hardware(&self) -> Hardware
    where
        M::Error: Into<crate::MemoryFailure>,
    {
        self.hardware_with(Then::new(
            Then::new(PrepareChunk::default(), ReadCopy::default()),
            Then::new(
                WriteCopy::default(),
                Then::new(CompletionInput, self.responses()),
            ),
        ))
    }
}
