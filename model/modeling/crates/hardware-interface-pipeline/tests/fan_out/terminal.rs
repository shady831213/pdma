use std::cell::Cell;

use hardware_interface_pipeline::{
    BranchError, DeliveryPlan, Distribute, FanOut, Operation, RunOutcome,
};

use super::fixtures::{Context, Destination, Dispatch, Parcel, Receipt, Report, Routing, dispatch};

#[test]
fn public_fan_out_state_threads_owned_payload_and_receipt() {
    use hardware_interface_pipeline::{FanOutState, Stage, StepOutcome};

    // Given: public state owns distinct input, metadata, payload, and receipt types.
    let mut hardware = FanOut::new(vec![Destination::new(0)], Routing);
    let mut context = Context::default();
    let state: FanOutState<Dispatch, String, Parcel, Parcel, Receipt> =
        <FanOut<Destination, Routing> as Stage<Context>>::start(dispatch(&[0]));

    // When: the caller explicitly passes each returned continuation to the next step.
    let StepOutcome::Progressed(state) = hardware.step(&mut context, state) else {
        panic!("planning must retain the owned delivery");
    };
    assert!(context.attempts.is_empty());
    let StepOutcome::Progressed(state) = hardware.step(&mut context, state) else {
        panic!("delivery must retain the owned receipt");
    };
    let StepOutcome::Complete(result) = hardware.step(&mut context, state) else {
        panic!("reduction must complete");
    };

    // Then: ownership reaches the reducer with one committed delivery.
    assert_eq!(
        result,
        Ok(Report {
            label: "delivery".into(),
            receipts: vec![Receipt(0)]
        })
    );
    assert_eq!(context.deliveries, [(0, "payload 0".into())]);
    assert_eq!(context.plans.get(), 1);
}

#[derive(Clone, Copy)]
enum Finish {
    Success,
    SplitError,
    CombineError,
}

struct Policy<'a> {
    finish: Finish,
    reductions: &'a Cell<usize>,
}

impl Distribute<Context, Destination> for Policy<'_> {
    type Input = Dispatch;
    type Output = Report;
    type State = String;

    fn split(
        &mut self,
        context: &Context,
        input: Dispatch,
    ) -> DeliveryPlan<String, Parcel, &'static str> {
        let plan = Routing.split(context, input)?;
        match self.finish {
            Finish::SplitError => Err(BranchError::Application("split failed")),
            Finish::Success | Finish::CombineError => Ok(plan),
        }
    }

    fn combine(
        &mut self,
        label: String,
        receipts: Vec<Receipt>,
    ) -> Result<Report, BranchError<&'static str>> {
        self.reductions.set(self.reductions.get() + 1);
        match self.finish {
            Finish::CombineError => Err(BranchError::Application("combine failed")),
            Finish::Success | Finish::SplitError => Routing.combine(label, receipts),
        }
    }
}

#[test]
fn operation_remains_terminal_after_success_or_policy_error() {
    // Given: public operation state with distinct input, metadata, child-state and output types.
    for finish in [Finish::Success, Finish::SplitError, Finish::CombineError] {
        let reductions = Cell::new(0);
        let policy = Policy {
            finish,
            reductions: &reductions,
        };
        let mut hardware = FanOut::new(vec![Destination::new(0)], policy);
        let mut context = Context::default();
        let mut state = Operation::new(dispatch(&[0]));

        // When: stepping finishes the request and the caller drives the completed operation again.
        let first = state.step(&mut hardware, &mut context);
        let (result, expected, deliveries) = match finish {
            Finish::SplitError => (first, Err(BranchError::Application("split failed")), 0),
            Finish::Success | Finish::CombineError => {
                assert_eq!(first, Ok(RunOutcome::Progressed));
                assert_eq!(
                    state.step(&mut hardware, &mut context),
                    Ok(RunOutcome::Progressed)
                );
                let expected = match finish {
                    Finish::Success => Ok(Report {
                        label: "delivery".into(),
                        receipts: vec![Receipt(0)],
                    }),
                    Finish::CombineError => Err(BranchError::Application("combine failed")),
                    Finish::SplitError => Err(BranchError::Application("split failed")),
                };
                (state.step(&mut hardware, &mut context), expected, 1)
            }
        };
        let repeated = state.step(&mut hardware, &mut context);

        // Then: neither planning, committed delivery nor reduction is replayed.
        assert_eq!(result, Ok(RunOutcome::Complete(expected)));
        assert_eq!(repeated, Err(hardware_interface_pipeline::AlreadyComplete));
        assert_eq!(context.plans.get(), 1);
        assert_eq!(context.attempts.len(), deliveries);
        assert_eq!(context.deliveries.len(), deliveries);
        assert_eq!(reductions.get(), deliveries);
    }
}
