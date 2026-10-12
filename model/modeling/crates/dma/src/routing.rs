use crate::{ChannelTask, Dma, Error, Publish, Resettable, UpdateIrq};
use hardware_interface_pipeline::{
    AfterEach, Dispatch, FanOut, FanState, Route, RunError, Stage, StepOutcome, StepResult,
};
use hardware_interface_runtime::MemoryAccess;
use pdma_model_contract::dma::Response;

/// A publication endpoint bound to its channel and reset generation.
pub type ResponsePort = Resettable<AfterEach<Publish, UpdateIrq>>;

/// Selects the publication port named by a completed response.
#[derive(Debug)]
pub struct RouteResponse;

impl Route<ChannelTask<Response>, Error> for RouteResponse {
    fn target(&mut self, input: &ChannelTask<Response>) -> Result<usize, Error> {
        Ok(input.channel().index())
    }
}

type ResponseFan = FanOut<ResponseValue, Dispatch<RouteResponse>, ResponsePort>;

/// Dispatches one response through the shared fan lifecycle.
#[derive(Debug)]
pub struct Responses(ResponseFan);

impl Responses {
    pub(crate) fn new(ports: Vec<ResponsePort>) -> Self {
        Self(FanOut::new(
            ResponseValue,
            Dispatch::new(RouteResponse),
            ports,
        ))
    }
}

impl<M: MemoryAccess<SB>, SB> Stage<Dma<M, SB>> for Responses {
    type Input = ChannelTask<Response>;
    type Output = ();
    type Error = Error;
    type State = Option<
        FanState<
            Option<Self::Input>,
            Self::Input,
            <ResponsePort as Stage<Dma<M, SB>>>::State,
            ChannelTask<()>,
        >,
    >;

    fn start(input: Self::Input) -> Self::State {
        <ResponseFan as Stage<Dma<M, SB>>>::start(input)
    }

    fn step(&mut self, device: &mut Dma<M, SB>, state: &mut Self::State) -> StepResult<(), Error> {
        Ok(match self.0.step(device, state)? {
            StepOutcome::InProgress => StepOutcome::InProgress,
            StepOutcome::Complete(result) => {
                StepOutcome::Complete(result.map(|_| ()).map_err(Error::from))
            }
        })
    }
}

#[derive(Debug)]
struct ResponseValue;

impl<M, SB> Stage<Dma<M, SB>> for ResponseValue {
    type Input = ChannelTask<Response>;
    type Output = ChannelTask<Response>;
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
        Ok(StepOutcome::Complete(Ok(state
            .take()
            .ok_or(RunError::Exhausted)?)))
    }
}
