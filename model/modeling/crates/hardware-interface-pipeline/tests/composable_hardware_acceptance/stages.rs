use std::convert::Infallible;

use hardware_interface_pipeline::{Stage, StepOutcome};

#[derive(Debug)]
pub struct Request {
    pub id: u64,
    pub bytes: Vec<u8>,
}

#[derive(Debug)]
pub struct Decoded {
    id: u64,
    bytes: Vec<u8>,
}

#[derive(Debug)]
pub struct Executed {
    id: u64,
    result: Result<Vec<u8>, Overflow>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Response {
    pub id: u64,
    pub result: Result<u32, Overflow>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Overflow;

#[derive(Debug, Default)]
pub struct Environment {
    pub writable: bool,
    pub responses: Vec<u64>,
}

#[derive(Debug, Default)]
pub struct Decode {
    pub accepted: usize,
}

#[derive(Debug)]
pub struct Compute {
    pub factor: u8,
    pub executed: usize,
}

#[derive(Debug)]
pub struct Publish;

impl Stage<Environment> for Decode {
    type Input = Request;
    type Output = Decoded;
    type Error = Infallible;
    type Wait = ();
    type State = Option<Request>;

    fn start(input: Request) -> Self::State {
        Some(input)
    }

    fn step(
        &mut self,
        _: &mut Environment,
        state: &mut Self::State,
    ) -> StepOutcome<Decoded, Infallible, ()> {
        let Some(Request { id, bytes }) = state.take() else {
            return StepOutcome::Exhausted;
        };
        self.accepted += 1;
        StepOutcome::Complete(Ok(Decoded { id, bytes }))
    }
}

impl Stage<Environment> for Compute {
    type Input = Decoded;
    type Output = Executed;
    type Error = Infallible;
    type Wait = ();
    type State = Option<Decoded>;

    fn start(input: Decoded) -> Self::State {
        Some(input)
    }

    fn step(
        &mut self,
        _: &mut Environment,
        state: &mut Self::State,
    ) -> StepOutcome<Executed, Infallible, ()> {
        let Some(Decoded { id, bytes }) = state.take() else {
            return StepOutcome::Exhausted;
        };
        self.executed += 1;
        let result = bytes
            .into_iter()
            .map(|byte| byte.checked_mul(self.factor).ok_or(Overflow))
            .collect();
        StepOutcome::Complete(Ok(Executed { id, result }))
    }
}

impl Stage<Environment> for Publish {
    type Input = Executed;
    type Output = Response;
    type Error = Infallible;
    type Wait = ();
    type State = Option<Executed>;

    fn start(input: Executed) -> Self::State {
        Some(input)
    }

    fn step(
        &mut self,
        environment: &mut Environment,
        state: &mut Self::State,
    ) -> StepOutcome<Response, Infallible, ()> {
        if !environment.writable {
            return StepOutcome::Pending(());
        }
        let Some(Executed { id, result }) = state.take() else {
            return StepOutcome::Exhausted;
        };
        environment.responses.push(id);
        StepOutcome::Complete(Ok(Response {
            id,
            result: result.map(|bytes| bytes.into_iter().map(u32::from).sum()),
        }))
    }
}
