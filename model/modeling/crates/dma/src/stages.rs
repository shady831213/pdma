use crate::{
    ChannelContext, CheckRanges, Dma, Error, Fetch, NextChunk, ReadCopy, ResetControl, Resettable,
    Responses, WriteCopy,
};
use hardware_interface_pipeline::{AfterEach, RunError, Stage, StepOutcome, StepResult, Then};
use hardware_interface_runtime::MemoryAccess;
use pdma_model_contract::dma::{ChannelId, Response};
use std::convert::Infallible;

/// Publish a retained response; a full ring suspends only this stage.
#[derive(Debug, Default)]
pub struct Publish;

impl<M: MemoryAccess<SB>, SB> Stage<ChannelContext<'_, M, SB>> for Publish {
    type Input = Response;
    type Output = ();
    type Error = Error;
    type State = Option<Response>;
    fn start(input: Response) -> Self::State {
        Some(input)
    }
    fn step(
        &mut self,
        context: &mut ChannelContext<'_, M, SB>,
        retained: &mut Self::State,
    ) -> StepResult<(), Error> {
        let response = retained.take().ok_or(RunError::Exhausted)?;
        if context.channel.resetting() {
            return Ok(StepOutcome::Complete(Err(Error::Reset)));
        }
        if context.halted() {
            return Ok(StepOutcome::Complete(Err(Error::Halted)));
        }
        if !context.channel.enabled() {
            *retained = Some(response);
            return Ok(StepOutcome::InProgress);
        }
        let was_empty = context.channel.response.pointers().is_empty();
        match context.channel.response.push(&context.memory, &response) {
            Ok(true) => {
                context.channel.published(&response, was_empty);
                Ok(StepOutcome::Complete(Ok(())))
            }
            Ok(false) => {
                *retained = Some(response);
                Ok(StepOutcome::InProgress)
            }
            Err(fatal) => {
                context.channel.latch(fatal);
                Ok(StepOutcome::Complete(Err(Error::ChannelFatal(fatal))))
            }
        }
    }
}

/// Refresh observable IRQ causes from queue-owned response and fatal state.
#[derive(Debug, Default)]
pub struct UpdateIrq;

impl<M, SB> Stage<ChannelContext<'_, M, SB>> for UpdateIrq {
    type Input = ();
    type Output = ();
    type Error = Infallible;
    type State = ();
    fn start((): ()) {}
    fn step(
        &mut self,
        context: &mut ChannelContext<'_, M, SB>,
        (): &mut (),
    ) -> StepResult<(), Infallible> {
        context.channel.update_irq();
        Ok(StepOutcome::Complete(Ok(())))
    }
}

/// One composable prepare/read/write iteration.
pub type CopyChunk = Then<Then<NextChunk, ReadCopy>, WriteCopy>;

/// Range validation followed by one bounded-memory copy chunk.
pub type Execution = Then<CheckRanges, CopyChunk>;

/// Shared chunk execution and optional terminal response publication.
pub type Pipeline = Then<Resettable<AfterEach<Execution, UpdateIrq>>, Responses>;

/// Finite shared service transaction with assembly-owned fault continuations.
pub type Service<M, SB = (), S = Pipeline> = AfterEach<
    crate::commands::Commands<Then<Fetch, S>, <Then<Fetch, S> as Stage<Dma<M, SB>>>::State>,
    ResetControl,
>;

impl<M, SB> Dma<M, SB> {
    /// Builds response dispatch with each destination bound to its owning channel.
    pub fn responses(&self) -> Responses {
        let ports = (0..self.channels.len())
            .map(|index| {
                Resettable::for_channel(ChannelId::new(index), AfterEach::new(Publish, UpdateIrq))
            })
            .collect();
        Responses::new(ports)
    }
}

impl<M: MemoryAccess<SB>, SB> Dma<M, SB> {
    /// Builds one finite command-chunk service transaction.
    pub fn service(&self) -> Service<M, SB>
    where
        M::Error: Into<crate::MemoryFailure>,
    {
        let chunk = Then::new(Then::new(NextChunk, ReadCopy), WriteCopy);
        let execution = Then::new(CheckRanges, chunk);
        self.service_with(Then::new(
            Resettable::new(AfterEach::new(execution, UpdateIrq)),
            self.responses(),
        ))
    }

    /// Builds finite service with one shared custom chunk composition.
    pub fn service_with<S>(&self, pipeline: S) -> Service<M, SB, S>
    where
        S: Stage<Self, Input = crate::ChannelTask<crate::CommandInput>, Output = (), Error = Error>,
    {
        AfterEach::new(
            crate::commands::Commands::new(self.channels.len(), Then::new(Fetch, pipeline)),
            ResetControl,
        )
    }
}
