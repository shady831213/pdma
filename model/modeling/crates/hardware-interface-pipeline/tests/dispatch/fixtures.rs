use hardware_interface_pipeline::{Route, RunError, Stage, StepOutcome, StepResult};
#[derive(Default)]
pub(super) struct Context {
    pub deliveries: Vec<(usize, String)>,
    pub attempts: Vec<usize>,
    pub source_calls: usize,
    pub released: bool,
}
#[derive(Debug)]
pub(super) struct Request {
    pub target: usize,
    pub payload: String,
}
pub(super) fn request(target: usize) -> Request {
    Request {
        target,
        payload: "owned payload".into(),
    }
}
pub(super) struct Source;
impl Stage<Context> for Source {
    type Input = Request;
    type Output = Request;
    type Error = &'static str;
    type State = Option<Request>;
    fn start(input: Request) -> Self::State {
        Some(input)
    }
    fn step(
        &mut self,
        context: &mut Context,
        state: &mut Self::State,
    ) -> StepResult<Request, Self::Error> {
        let value = state.take().ok_or(RunError::Exhausted)?;
        context.source_calls += 1;
        Ok(StepOutcome::Complete(Ok(value)))
    }
}
pub(super) enum Behavior {
    Complete,
    Stall,
    Progress,
    Error,
    Exhausted,
}
pub(super) struct Destination {
    pub id: usize,
    pub behavior: Behavior,
}
impl Destination {
    pub(super) const fn new(id: usize) -> Self {
        Self {
            id,
            behavior: Behavior::Complete,
        }
    }
}
impl Stage<Context> for Destination {
    type Input = Request;
    type Output = ();
    type Error = &'static str;
    type State = (Option<Request>, bool);
    fn start(input: Request) -> Self::State {
        (Some(input), false)
    }
    fn step(
        &mut self,
        context: &mut Context,
        state: &mut Self::State,
    ) -> StepResult<(), Self::Error> {
        context.attempts.push(self.id);
        match self.behavior {
            Behavior::Stall if !context.released => return Ok(StepOutcome::InProgress),
            Behavior::Progress if !state.1 => {
                state
                    .0
                    .as_mut()
                    .ok_or(RunError::Exhausted)?
                    .payload
                    .push_str(" advanced");
                state.1 = true;
                return Ok(StepOutcome::InProgress);
            }
            Behavior::Error => {
                state.0 = None;
                return Ok(StepOutcome::Complete(Err("destination failed")));
            }
            Behavior::Exhausted => {
                state.0 = None;
                return Err(RunError::Exhausted);
            }
            Behavior::Complete | Behavior::Stall | Behavior::Progress => {}
        }
        let value = state.0.take().ok_or(RunError::Exhausted)?;
        context.deliveries.push((self.id, value.payload));
        Ok(StepOutcome::Complete(Ok(())))
    }
}
pub(super) enum Routing {
    Accept,
    Reject,
}
impl Route<Request, &'static str> for Routing {
    fn target(&mut self, input: &Request) -> Result<usize, &'static str> {
        match self {
            Self::Accept => Ok(input.target),
            Self::Reject => Err("route failed"),
        }
    }
}
pub(super) fn finish<S: Stage<Context>>(
    hardware: &mut S,
    state: &mut S::State,
    context: &mut Context,
) -> StepResult<S::Output, S::Error> {
    for _ in 0..100 {
        match hardware.step(context, state) {
            Ok(StepOutcome::InProgress) => {}
            terminal => return terminal,
        }
    }
    Err(RunError::BudgetExhausted)
}
