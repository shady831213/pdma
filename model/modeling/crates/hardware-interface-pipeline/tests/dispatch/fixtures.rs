use std::cell::Cell;

use hardware_interface_pipeline::{Route, Stage, StepOutcome};

#[derive(Default)]
pub(super) struct Context {
    pub deliveries: Vec<(usize, String)>,
    pub attempts: Vec<usize>,
    pub routes: Cell<usize>,
    pub released: bool,
}

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

pub(super) struct Parcel(pub String);

pub(super) struct Receipt {
    _payload: String,
}

#[derive(Clone, Copy)]
pub(super) enum Behavior {
    Complete,
    Pending,
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
    type Input = Parcel;
    type Output = Receipt;
    type Error = &'static str;
    type Wait = &'static str;
    type State = (Option<Parcel>, bool);

    fn start(input: Parcel) -> Self::State {
        (Some(input), false)
    }

    fn step(
        &mut self,
        context: &mut Context,
        state: &mut Self::State,
    ) -> StepOutcome<Receipt, &'static str, &'static str> {
        context.attempts.push(self.id);
        match self.behavior {
            Behavior::Pending if !context.released => return StepOutcome::Pending("release"),
            Behavior::Progress if !state.1 => {
                let Some(Parcel(payload)) = state.0.as_mut() else {
                    return StepOutcome::Exhausted;
                };
                payload.push_str(" advanced");
                state.1 = true;
                return StepOutcome::Progressed;
            }
            Behavior::Error => return StepOutcome::Complete(Err("destination failed")),
            Behavior::Exhausted => return StepOutcome::Exhausted,
            Behavior::Complete | Behavior::Pending | Behavior::Progress => {}
        }
        let Some(Parcel(payload)) = state.0.take() else {
            return StepOutcome::Exhausted;
        };
        context.deliveries.push((self.id, payload));
        StepOutcome::Complete(Ok(Receipt {
            _payload: "owned receipt".into(),
        }))
    }
}

pub(super) enum Routing {
    Accept,
    Reject,
}

impl Route<Context, Destination> for Routing {
    type Input = Request;

    fn route(
        &mut self,
        context: &Context,
        input: Request,
    ) -> Result<(usize, Parcel), &'static str> {
        context.routes.set(context.routes.get() + 1);
        match self {
            Self::Accept => Ok((input.target, Parcel(format!("routed {}", input.payload)))),
            Self::Reject => Err("route failed"),
        }
    }
}
