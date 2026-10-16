use crate::{Completion, Error, ExecutionContext, Flow};
use hardware_interface_pipeline::{
    Dispatch, FanOut, Route, RunError, Stage, StepOutcome, StepResult,
};
use hardware_interface_runtime::{BoundMemory, MemoryAccess};
use pdma_model_contract::dma::{ChannelId, ContextId, DmaAccess};

#[cfg(test)]
mod tests;

/// Publishes the selected channel's retained response before retiring its work.
#[derive(Debug)]
pub struct Publish {
    channel: ChannelId,
    state: Option<Flow<Completion>>,
}
impl Publish {
    /// Binds publication to the specified response-ring owner.
    pub const fn new(channel: ChannelId) -> Self {
        Self {
            channel,
            state: None,
        }
    }
}
impl<M: MemoryAccess<SB>, SB> Stage<ExecutionContext<'_, M, SB>> for Publish {
    type Input = Flow<Completion>;
    type Output = Flow<()>;
    type Error = std::convert::Infallible;
    fn step(
        &mut self,
        context: &mut ExecutionContext<'_, M, SB>,
        input: &mut Option<Self::Input>,
    ) -> StepResult<Self::Output, Self::Error> {
        if self.state.is_none() {
            self.state = input.take();
        }
        let channel = self.channel;
        Flow::advance(&mut self.state, context, |state, context| {
            let completion = state.as_ref().ok_or(RunError::Exhausted)?;
            let work = completion.work;
            if work.channel() != channel {
                *state = None;
                return Ok(StepOutcome::Complete(Err(Error::Routing)));
            }
            match context.ready(work) {
                Err(error) => {
                    *state = None;
                    return Ok(StepOutcome::Complete(Err(error)));
                }
                Ok(false) => return Ok(StepOutcome::InProgress),
                Ok(true) => {}
            }
            let memory = BoundMemory::new(
                context.memory,
                (context.sideband)(DmaAccess {
                    channel: work.channel,
                    context: ContextId::new(context.selected.registers.values.ring_context.raw()),
                }),
            );
            let was_empty = context.selected.response.pointers().is_empty();
            match context
                .selected
                .response
                .push(&memory, &completion.response)
            {
                Ok(true) => {
                    context.selected.published(&completion.response, was_empty);
                    context.selected.work = None;
                    if *context.active == Some(work) {
                        *context.active = None;
                    }
                    context.changed();
                    *state = None;
                    Ok(StepOutcome::Complete(Ok(Some(()))))
                }
                Ok(false) => Ok(StepOutcome::InProgress),
                Err(fatal) => {
                    context.selected.latch(fatal);
                    context.changed();
                    *state = None;
                    Ok(StepOutcome::Complete(Err(Error::ChannelFatal(fatal))))
                }
            }
        })
    }
}

/// Routes a response-ready work snapshot to its owning channel.
#[derive(Debug)]
struct RouteResponse;

impl Route<Flow<Completion>, std::convert::Infallible> for RouteResponse {
    fn target(&mut self, input: &Flow<Completion>) -> Result<usize, std::convert::Infallible> {
        Ok(input.work.map_or(0, |work| work.channel().index()))
    }
}

type ResponseFan = FanOut<
    ResponseValue,
    Dispatch<RouteResponse>,
    Publish,
    Flow<Completion>,
    Flow<Completion>,
    Flow<Completion>,
    Flow<()>,
    std::convert::Infallible,
>;

/// Dispatches owned response data through shared `FanOut` wiring.
#[derive(Debug)]
pub struct Responses {
    fan: ResponseFan,
    input: Option<Flow<Completion>>,
    owner: Option<crate::Work>,
    busy: bool,
}

impl Responses {
    pub(crate) fn new(ports: Vec<Publish>) -> Self {
        Self {
            fan: FanOut::new(ResponseValue, Dispatch::new(RouteResponse), ports),
            input: None,
            owner: None,
            busy: false,
        }
    }
}

impl<M: MemoryAccess<SB>, SB> Stage<ExecutionContext<'_, M, SB>> for Responses {
    type Input = Flow<Completion>;
    type Output = Flow<()>;
    type Error = std::convert::Infallible;

    fn step(
        &mut self,
        device: &mut ExecutionContext<'_, M, SB>,
        input: &mut Option<Self::Input>,
    ) -> StepResult<Self::Output, Self::Error> {
        if !self.busy {
            let Some(input) = input.take() else {
                return Ok(StepOutcome::InProgress);
            };
            if input.data().is_none() {
                return Ok(StepOutcome::Complete(Ok(input.finish())));
            }
            self.owner = input.work;
            self.input = Some(input);
            self.busy = true;
        }
        let result = self.fan.step(device, &mut self.input);
        match result {
            Ok(StepOutcome::InProgress) => Ok(StepOutcome::InProgress),
            Ok(StepOutcome::Complete(result)) => {
                self.busy = false;
                self.input = None;
                let owner = self.owner.take();
                let output = result.map_or_else(
                    |_| Flow::new(owner, Err(crate::Error::Routing)),
                    |mut outputs| {
                        outputs.pop().map_or_else(
                            || Flow::new(owner, Err(crate::Error::Routing)),
                            |(_, output)| output,
                        )
                    },
                );
                Ok(StepOutcome::Complete(Ok(output)))
            }
            Err(error) => Err(error),
        }
    }
}

#[derive(Debug)]
struct ResponseValue;

impl<M, SB> Stage<ExecutionContext<'_, M, SB>> for ResponseValue {
    type Input = Flow<Completion>;
    type Output = Flow<Completion>;
    type Error = std::convert::Infallible;
    fn step(
        &mut self,
        _: &mut ExecutionContext<'_, M, SB>,
        input: &mut Option<Self::Input>,
    ) -> StepResult<Self::Output, Self::Error> {
        Ok(input.take().map_or(StepOutcome::InProgress, |completion| {
            StepOutcome::Complete(Ok(completion))
        }))
    }
}
