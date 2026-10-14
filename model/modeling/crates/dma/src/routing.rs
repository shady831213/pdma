use crate::{Dma, Error, Publish, Work};
use hardware_interface_pipeline::{
    Dispatch, FanOut, FanState, Route, RunError, Stage, StepOutcome, StepResult,
};
use hardware_interface_runtime::MemoryAccess;

/// Routes a response-ready work snapshot to its owning channel.
#[derive(Debug)]
struct RouteResponse;

impl Route<Work, Error> for RouteResponse {
    fn target(&mut self, input: &Work) -> Result<usize, Error> {
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

impl<M: MemoryAccess<SB>, SB> Stage<Dma<M, SB>> for Responses {
    type Input = Option<Work>;
    type Output = ();
    type Error = Error;
    type State = Option<Option<FanState<Option<Work>, Work, Option<Work>, ()>>>;

    fn start(input: Self::Input) -> Self::State {
        Some(input.and_then(<ResponseFan as Stage<Dma<M, SB>>>::start))
    }

    fn step(&mut self, device: &mut Dma<M, SB>, state: &mut Self::State) -> StepResult<(), Error> {
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
                    result.map(|_| ()).map_err(Error::from),
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

impl<M, SB> Stage<Dma<M, SB>> for ResponseValue {
    type Input = Work;
    type Output = Work;
    type Error = Error;
    type State = Option<Work>;
    fn start(input: Work) -> Self::State {
        Some(input)
    }
    fn step(&mut self, _: &mut Dma<M, SB>, state: &mut Self::State) -> StepResult<Work, Error> {
        Ok(StepOutcome::Complete(Ok(state
            .take()
            .ok_or(RunError::Exhausted)?)))
    }
}
