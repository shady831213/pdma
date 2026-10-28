use crate::channel::CopyState;
use crate::{COPY_CHUNK_BYTES, Error, ExecutionContext, Work};
use hw_model_io::memory::MemoryAddress;
use hw_model_pipeline::{StepOutcome, StepResult};
use pdma_model_contract::dma::{ChannelId, Command, ContextId};
use std::convert::Infallible;

/// Execution data retaining identity through bubbles and domain stops until retirement.
#[derive(Debug)]
#[must_use]
pub struct Flow<T> {
    pub(crate) work: Option<Work>,
    pub(crate) value: Result<Option<T>, Error>,
}

impl<T> Flow<T> {
    pub(crate) const fn new(work: Option<Work>, value: Result<Option<T>, Error>) -> Self {
        Self { work, value }
    }
    /// Borrows actual data; bubbles and domain stops contain no data.
    pub fn data(&self) -> Option<&T> {
        self.value.as_ref().ok().and_then(Option::as_ref)
    }
    /// Mutably borrows data without exposing task identity.
    pub fn data_mut(&mut self) -> Option<&mut T> {
        self.value.as_mut().ok().and_then(Option::as_mut)
    }
    /// Transforms data while retaining bubbles, domain stops and identity.
    pub fn map<U>(self, transform: impl FnOnce(T) -> U) -> Flow<U> {
        Flow::new(self.work, self.value.map(|data| data.map(transform)))
    }
    /// Carries a domain stop to retirement after releasing this value's data.
    pub fn stop<U>(self, error: Error) -> Flow<U> {
        let error = match self.value {
            Err(retained) => retained,
            Ok(_) => error,
        };
        Flow::new(self.work, Err(error))
    }
    /// Completes a custom tail while retaining any preceding domain stop.
    pub fn finish(self) -> Flow<Completion> {
        let work = self.work;
        Flow::new(
            work,
            self.value.and_then(|value| {
                value
                    .map(|_| work.map(|work| Completion { work }).ok_or(Error::Routing))
                    .transpose()
            }),
        )
    }
    /// Reports the retained domain outcome without releasing ownership.
    pub const fn status(&self) -> Result<(), Error> {
        match self.value {
            Ok(_) => Ok(()),
            Err(error) => Err(error),
        }
    }
    /// Observes this value's retained stop and current owner lifecycle.
    ///
    /// # Errors
    /// Returns the retained stop, or a current reset/halt error.
    pub fn lifecycle<M, S>(&self, context: &mut ExecutionContext<'_, M, S>) -> Result<(), Error> {
        match &self.value {
            Err(error) => Err(*error),
            Ok(_) => self
                .work
                .map_or(Ok(()), |work| context.select(work)?.lifecycle()),
        }
    }
    pub(crate) fn advance<M, S, U>(
        state: &mut Option<Self>,
        context: &mut ExecutionContext<'_, M, S>,
        operation: impl FnOnce(
            &mut Option<T>,
            &mut crate::SelectedResources<'_, M, S>,
        ) -> StepResult<Option<U>, Error>,
    ) -> StepResult<Flow<U>, Infallible> {
        let Some(flow) = state.as_mut() else {
            return Ok(StepOutcome::InProgress);
        };
        let work = flow.work;
        let result = match (&mut flow.value, work) {
            (Err(error), _) => StepOutcome::Complete(Err(*error)),
            (Ok(_), None) => StepOutcome::Complete(Ok(None)),
            (Ok(data), Some(identity)) => match context.select(identity) {
                Ok(mut selected) => operation(data, &mut selected)?,
                Err(error) => StepOutcome::Complete(Err(error)),
            },
        };
        match result {
            StepOutcome::InProgress => Ok(StepOutcome::InProgress),
            StepOutcome::Complete(value) => {
                *state = None;
                Ok(StepOutcome::Complete(Ok(Flow::new(work, value))))
            }
        }
    }
}

/// Non-owning observation of a channel head, refreshed before descriptor acceptance.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CommandSnapshot {
    pub(crate) work: Work,
    pub(crate) consumer: u32,
    pub(crate) address: u64,
}

/// Whether an accepted descriptor needs initial validation or resumes retained work.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CommandAdmission {
    /// Newly consumed from the command ring.
    Fresh,
    /// Already accepted, with channel-owned progress retained.
    Resume,
}

/// Accepted descriptor transferred into opcode dispatch.
#[derive(Debug)]
pub struct FetchedCommand {
    pub(crate) work: Work,
    pub(crate) command: Command,
    pub(crate) admission: CommandAdmission,
}

impl FetchedCommand {
    /// Whether execution is fresh or resumes retained hardware state.
    pub const fn admission(&self) -> CommandAdmission {
        self.admission
    }
    /// The accepted immutable descriptor.
    pub const fn command(&self) -> &Command {
        &self.command
    }
    /// Its event-ring owner.
    pub const fn channel(&self) -> ChannelId {
        self.work.channel()
    }
}

/// Validated command traversal transferred into channel-owned execution state.
#[derive(Debug)]
pub struct CopyProgress {
    pub(crate) work: Work,
    pub(crate) progress: CopyState,
}

/// A request for the next bounded traversal pass.
#[derive(Debug)]
pub struct ChunkInput {
    pub(crate) work: Work,
}

impl ChunkInput {
    pub(crate) const fn next(work: Work) -> Self {
        Self { work }
    }
    /// Channel whose retained traversal or read checkpoint is requested.
    pub const fn channel(&self) -> ChannelId {
        self.work.channel()
    }
}

/// Bounded source/destination span and its authoritative command traversal.
#[derive(Debug)]
pub struct Chunk {
    pub(crate) work: Work,
    pub(crate) source: MemoryAddress,
    pub(crate) destination: MemoryAddress,
    pub(crate) length: usize,
    pub(crate) source_context: u32,
    pub(crate) destination_context: u32,
}

impl Chunk {
    /// Source byte address for this interaction.
    pub const fn source(&self) -> MemoryAddress {
        self.source
    }
    /// Destination byte address for this interaction.
    pub const fn destination(&self) -> MemoryAddress {
        self.destination
    }
    /// Number of payload bytes in this interaction.
    pub const fn len(&self) -> usize {
        self.length
    }
    /// Whether the command requires no payload interaction.
    pub const fn is_empty(&self) -> bool {
        self.length == 0
    }
    /// Source address-space selector.
    pub const fn source_context(&self) -> ContextId {
        ContextId::new(self.source_context)
    }
    /// Destination address-space selector.
    pub const fn destination_context(&self) -> ContextId {
        ContextId::new(self.destination_context)
    }
}

/// Successfully read bytes, owned by the dataflow until write or fault parking.
#[derive(Debug)]
pub struct Payload {
    pub(crate) chunk: Chunk,
    pub(crate) bytes: [u8; COPY_CHUNK_BYTES],
}

impl Payload {
    /// The bounded transfer descriptor.
    pub const fn chunk(&self) -> &Chunk {
        &self.chunk
    }
    /// Active payload bytes, with length fixed by traversal.
    pub fn bytes(&self) -> Result<&[u8], crate::Error> {
        self.bytes
            .get(..self.chunk.length)
            .ok_or(crate::Error::Routing)
    }
    /// Transform the actual bytes consumed by the write stage.
    pub fn bytes_mut(&mut self) -> Result<&mut [u8], crate::Error> {
        self.bytes
            .get_mut(..self.chunk.length)
            .ok_or(crate::Error::Routing)
    }
}

/// Terminal command success bound to its accepted owner.
#[derive(Debug, PartialEq, Eq)]
pub struct Completion {
    pub(crate) work: Work,
}

impl Completion {
    /// Channel whose command completed successfully.
    pub const fn channel(&self) -> ChannelId {
        self.work.channel()
    }
}
