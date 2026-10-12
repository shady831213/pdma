use crate::{
    ChannelContext, CheckRanges, CommandArbiter, Dma, Error, NextChunk, ReadCopy, ResetControl,
    Resettable, Responses, WriteCopy,
};
use hardware_interface_pipeline::{
    AfterEach, Repeat, RunError, Stage, StepOutcome, StepResult, Then,
};
use hardware_interface_runtime::MemoryAccess;
use pdma_model_contract::dma::{ChannelId, Response};
use std::{convert::Infallible, ops::ControlFlow};

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

/// Ends one command after completion or channel-local termination.
#[derive(Debug)]
pub struct RunCommand<S = Pipeline> {
    pipeline: S,
}

impl<S> RunCommand<S> {
    /// Runs one composed command with shared execution ownership and terminal cleanup.
    pub const fn new(pipeline: S) -> Self {
        Self { pipeline }
    }
}

impl<M: MemoryAccess<SB>, S, SB> Stage<Dma<M, SB>> for RunCommand<S>
where
    S: Stage<Dma<M, SB>, Output = (), Error = Error>,
{
    type Input = S::Input;
    type Output = ControlFlow<(), ()>;
    type Error = Error;
    type State = Option<S::State>;

    fn start(input: Self::Input) -> Self::State {
        Some(S::start(input))
    }

    fn step(
        &mut self,
        device: &mut Dma<M, SB>,
        state: &mut Self::State,
    ) -> StepResult<Self::Output, Error> {
        let Some(child) = state.as_mut() else {
            return Err(RunError::Exhausted);
        };
        let outcome = self.pipeline.step(device, child);
        if !matches!(outcome, Ok(StepOutcome::InProgress)) {
            *state = None;
            device.active = None;
        }
        device.refresh_maintenance();
        Ok(match outcome? {
            StepOutcome::Complete(
                Ok(()) | Err(Error::Reset | Error::ChannelFatal(_) | Error::Halted),
            ) => StepOutcome::Complete(Ok(ControlFlow::Continue(()))),
            StepOutcome::Complete(Err(error)) => StepOutcome::Complete(Err(error)),
            StepOutcome::InProgress => StepOutcome::InProgress,
        })
    }
}

/// One composable prepare/read/write iteration.
pub type CopyLoop = Then<Then<NextChunk, ReadCopy>, WriteCopy>;

/// Length validation followed by bounded-memory copy iterations.
pub type Execution = Then<CheckRanges, Repeat<CopyLoop>>;

/// Shared execution and publication hardware for one granted command.
pub type Pipeline = Then<Resettable<AfterEach<Execution, UpdateIrq>>, Responses>;

/// Continuously executes commands through one shared pipeline.
pub type CommandLoop = AfterEach<CommandArbiter<Pipeline>, ResetControl>;

impl<M, SB> Dma<M, SB> {
    /// Builds command arbitration independently of the downstream transform.
    pub fn intake(&self) -> crate::Intake {
        crate::Intake::new(self.channels.len())
    }

    /// Builds response dispatch with each destination bound to its owning channel.
    pub fn responses(&self) -> Responses {
        let ports = (0..self.channels.len())
            .map(|index| {
                Resettable::for_channel(ChannelId::new(index), AfterEach::new(Publish, UpdateIrq))
            })
            .collect();
        Responses::new(ports)
    }

    /// Builds the sustained command loop from this instance's hardware stages.
    pub fn command_loop(&self) -> CommandLoop {
        let chunk = Then::new(Then::new(NextChunk, ReadCopy), WriteCopy);
        let execution = Then::new(CheckRanges, Repeat::new(chunk));
        AfterEach::new(
            self.arbitrate(Then::new(
                Resettable::new(AfterEach::new(execution, UpdateIrq)),
                self.responses(),
            )),
            ResetControl,
        )
    }

    /// Arbitrates new and resumed commands through one shared downstream composition.
    pub fn arbitrate<S>(&self, pipeline: S) -> CommandArbiter<S> {
        CommandArbiter::new(self.channels.len(), pipeline)
    }
}
