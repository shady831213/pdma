mod candidate;
mod data_path;
mod execution;
mod fetch;
mod opcodes;
mod operations;
mod retire;
#[cfg(test)]
mod tests;

use crate::{Chunk, ChunkInput, Completion, Dma, ExecutionContext, Flow, Payload};
pub use candidate::{Candidate, CommandRequest, RetainedTask};
pub use data_path::Hardware;
pub use execution::{CheckRanges, CommandExecution, PrepareChunk, ReadCopy, WriteCopy};
pub use fetch::Fetch;
use hardware_interface_pipeline::{Stage, Then};
pub use retire::Retire;

/// One concrete chunk transferred through preparation, read and write.
pub type CopyChunk = Then<PrepareChunk, Then<ReadCopy, WriteCopy, Flow<Payload>>, Flow<Chunk>>;
/// Execution of an accepted command using bounded chunk stages.
pub type Execution = CommandExecution<CopyChunk>;
/// Default command execution through bounded Copy chunks.
pub type Pipeline = Execution;
impl<M, SB> Dma<M, SB> {
    /// Wires one finite opaque execution graph over selected channel resources.
    pub fn hardware_with<S>(&self, pipeline: S) -> Hardware<S>
    where
        for<'a> S: Stage<
                ExecutionContext<'a, M, SB>,
                Input = Flow<ChunkInput>,
                Output = Flow<Completion>,
                Error = std::convert::Infallible,
            >,
    {
        Hardware::new(pipeline, self.channels.len())
    }
}
impl<M: hardware_interface_io::memory::MemoryAccess<SB>, SB> Dma<M, SB> {
    /// Builds the finite connected command admission and bounded execution graph.
    pub fn hardware(&self) -> Hardware
    where
        M::Error: Into<crate::MemoryFailure>,
    {
        self.hardware_with(CommandExecution::new(Then::new(
            PrepareChunk::default(),
            Then::new(ReadCopy::default(), WriteCopy::default()),
        )))
    }
}
