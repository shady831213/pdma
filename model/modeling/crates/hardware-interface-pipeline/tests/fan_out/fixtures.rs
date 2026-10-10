use std::cell::Cell;

use hardware_interface_pipeline::{BranchError, DeliveryPlan, Distribute, Stage, StepOutcome};

#[derive(Default)]
pub(super) struct Context {
    pub deliveries: Vec<(usize, String)>,
    pub attempts: Vec<usize>,
    pub plans: Cell<usize>,
    pub released: bool,
}

pub(super) struct Parcel(pub String);

#[derive(Debug, PartialEq, Eq)]
pub(super) struct Receipt(pub usize);

#[derive(Clone, Copy)]
pub(super) enum Behavior {
    Complete,
    Pending,
    Error,
    Missing,
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
    type State = Parcel;

    fn start(input: Parcel) -> Self::State {
        input
    }

    fn step(
        &mut self,
        context: &mut Context,
        state: Self::State,
    ) -> StepOutcome<Self::State, Receipt, &'static str, &'static str> {
        context.attempts.push(self.id);
        match self.behavior {
            Behavior::Pending if !context.released => {
                return StepOutcome::Pending(state, "release");
            }
            Behavior::Error => return StepOutcome::Complete(Err("destination failed")),
            Behavior::Missing => return StepOutcome::Complete(Err("missing destination work")),
            Behavior::Complete | Behavior::Pending => {}
        }
        let Parcel(payload) = state;
        context.deliveries.push((self.id, payload));
        StepOutcome::Complete(Ok(Receipt(self.id)))
    }
}

pub(super) struct Dispatch {
    pub label: String,
    pub deliveries: Vec<(usize, Parcel)>,
}

#[derive(Debug, PartialEq, Eq)]
pub(super) struct Report {
    pub label: String,
    pub receipts: Vec<Receipt>,
}

pub(super) struct Routing;

impl Distribute<Context, Destination> for Routing {
    type Input = Dispatch;
    type Output = Report;
    type State = String;

    fn split(
        &mut self,
        context: &Context,
        input: Dispatch,
    ) -> DeliveryPlan<String, Parcel, &'static str> {
        context.plans.set(context.plans.get() + 1);
        Ok((input.label, input.deliveries))
    }

    fn combine(
        &mut self,
        label: String,
        receipts: Vec<Receipt>,
    ) -> Result<Report, BranchError<&'static str>> {
        Ok(Report { label, receipts })
    }
}

pub(super) fn dispatch(indices: &[usize]) -> Dispatch {
    Dispatch {
        label: "delivery".into(),
        deliveries: indices
            .iter()
            .map(|&index| (index, Parcel(format!("payload {index}"))))
            .collect(),
    }
}
