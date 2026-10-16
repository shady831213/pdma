use crate::{Completion, ExecutionContext, Publish};
use hardware_interface_pipeline::{Dispatch, FanOut, Route, Stage, StepOutcome, StepResult};
use hardware_interface_runtime::MemoryAccess;

/// Routes a response-ready work snapshot to its owning channel.
#[derive(Debug)]
struct RouteResponse;

impl Route<Completion, crate::Error> for RouteResponse {
    fn target(&mut self, input: &Completion) -> Result<usize, crate::Error> {
        Ok(input.channel().index())
    }
}

type ResponseFan = FanOut<
    ResponseValue,
    Dispatch<RouteResponse>,
    Publish,
    Completion,
    Completion,
    Completion,
    (),
    crate::Error,
>;

/// Dispatches an optional response-ready snapshot through shared `FanOut` wiring.
#[derive(Debug)]
pub struct Responses {
    fan: ResponseFan,
    input: Option<Completion>,
    busy: bool,
}

impl Responses {
    pub(crate) fn new(ports: Vec<Publish>) -> Self {
        Self {
            fan: FanOut::new(ResponseValue, Dispatch::new(RouteResponse), ports),
            input: None,
            busy: false,
        }
    }
}

impl<M: MemoryAccess<SB>, SB> Stage<ExecutionContext<'_, M, SB>> for Responses {
    type Input = Option<Completion>;
    type Output = ();
    type Error = crate::Error;

    fn step(
        &mut self,
        device: &mut ExecutionContext<'_, M, SB>,
        input: &mut Option<Self::Input>,
    ) -> StepResult<(), crate::Error> {
        if !self.busy {
            let Some(input) = input.take() else {
                return Ok(StepOutcome::InProgress);
            };
            let Some(completion) = input else {
                return Ok(StepOutcome::Complete(Ok(())));
            };
            self.input = Some(completion);
            self.busy = true;
        }
        let result = self.fan.step(device, &mut self.input);
        if matches!(result, Ok(StepOutcome::Complete(_))) {
            self.busy = false;
            self.input = None;
        }
        match result {
            Ok(StepOutcome::InProgress) => Ok(StepOutcome::InProgress),
            Ok(StepOutcome::Complete(result)) => Ok(StepOutcome::Complete(
                result.map(|_| ()).map_err(crate::Error::from),
            )),
            Err(error) => Err(error),
        }
    }
}

#[derive(Debug)]
struct ResponseValue;

impl<M, SB> Stage<ExecutionContext<'_, M, SB>> for ResponseValue {
    type Input = Completion;
    type Output = Completion;
    type Error = crate::Error;
    fn step(
        &mut self,
        _: &mut ExecutionContext<'_, M, SB>,
        input: &mut Option<Self::Input>,
    ) -> StepResult<Completion, crate::Error> {
        Ok(input.take().map_or(StepOutcome::InProgress, |completion| {
            StepOutcome::Complete(Ok(completion))
        }))
    }
}
