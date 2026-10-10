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
    type State = Request;

    fn start(input: Request) -> Self::State {
        input
    }

    fn step(
        &mut self,
        _: &mut Environment,
        state: Self::State,
    ) -> StepOutcome<Self::State, Decoded, Infallible, ()> {
        let Request { id, bytes } = state;
        self.accepted += 1;
        StepOutcome::Complete(Ok(Decoded { id, bytes }))
    }
}

impl Stage<Environment> for Compute {
    type Input = Decoded;
    type Output = Executed;
    type Error = Infallible;
    type Wait = ();
    type State = Decoded;

    fn start(input: Decoded) -> Self::State {
        input
    }

    fn step(
        &mut self,
        _: &mut Environment,
        state: Self::State,
    ) -> StepOutcome<Self::State, Executed, Infallible, ()> {
        let Decoded { id, bytes } = state;
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
    type State = Executed;

    fn start(input: Executed) -> Self::State {
        input
    }

    fn step(
        &mut self,
        environment: &mut Environment,
        state: Self::State,
    ) -> StepOutcome<Self::State, Response, Infallible, ()> {
        if !environment.writable {
            return StepOutcome::Pending(state, ());
        }
        let Executed { id, result } = state;
        environment.responses.push(id);
        StepOutcome::Complete(Ok(Response {
            id,
            result: result.map(|bytes| bytes.into_iter().map(u32::from).sum()),
        }))
    }
}
