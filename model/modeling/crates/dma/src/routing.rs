use crate::{Completion, ExecutionContext, Publish};
use hardware_interface_pipeline::{
    Dispatch, FanOut, FanState, Route, RunError, Stage, StepOutcome, StepResult,
};
use hardware_interface_runtime::MemoryAccess;

/// Routes a response-ready work snapshot to its owning channel.
#[derive(Debug)]
struct RouteResponse;

impl Route<Completion, crate::ExecutionError> for RouteResponse {
    fn target(&mut self, input: &Completion) -> Result<usize, crate::ExecutionError> {
        Ok(input.channel().index())
    }
}

type ResponseFan = FanOut<ResponseValue, Dispatch<RouteResponse>, Publish>;

/// Dispatches an optional response-ready snapshot through shared `FanOut` wiring.
#[derive(Debug)]
pub struct Responses(ResponseFan);

impl Responses {
    pub(crate) fn new(ports: Vec<Publish>) -> Self {
        Self(FanOut::new(
            ResponseValue,
            Dispatch::new(RouteResponse),
            ports,
        ))
    }
}

impl<M: MemoryAccess<SB>, SB> Stage<ExecutionContext<'_, M, SB>> for Responses {
    type Input = Option<Completion>;
    type Output = ();
    type Error = crate::ExecutionError;
    type State = Option<Option<FanState<Option<Completion>, Completion, Option<Completion>, ()>>>;

    fn start(input: Self::Input) -> Self::State {
        Some(input.and_then(<ResponseFan as Stage<ExecutionContext<'_, M, SB>>>::start))
    }

    fn step(
        &mut self,
        device: &mut ExecutionContext<'_, M, SB>,
        state: &mut Self::State,
    ) -> StepResult<(), crate::ExecutionError> {
        let Some(fan) = state.as_mut() else {
            return Err(RunError::Exhausted);
        };
        if fan.is_none() {
            *state = None;
            return Ok(StepOutcome::Complete(Ok(())));
        }
        match self.0.step(device, fan) {
            Ok(StepOutcome::InProgress) => Ok(StepOutcome::InProgress),
            Ok(StepOutcome::Complete(result)) => {
                *state = None;
                Ok(StepOutcome::Complete(
                    result.map(|_| ()).map_err(crate::ExecutionError::from),
                ))
            }
            Err(error) => {
                *state = None;
                Err(error)
            }
        }
    }
}

#[derive(Debug)]
struct ResponseValue;

impl<M, SB> Stage<ExecutionContext<'_, M, SB>> for ResponseValue {
    type Input = Completion;
    type Output = Completion;
    type Error = crate::ExecutionError;
    type State = Option<Completion>;
    fn start(input: Completion) -> Self::State {
        Some(input)
    }
    fn step(
        &mut self,
        _: &mut ExecutionContext<'_, M, SB>,
        state: &mut Self::State,
    ) -> StepResult<Completion, crate::ExecutionError> {
        Ok(StepOutcome::Complete(Ok(state
            .take()
            .ok_or(RunError::Exhausted)?)))
    }
}
