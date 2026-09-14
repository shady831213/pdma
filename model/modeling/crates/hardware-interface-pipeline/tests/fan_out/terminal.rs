use std::cell::Cell;

use hardware_interface_pipeline::{
    BranchError, DeliveryPlan, Distribute, FanOut, FanOutState, Stage, StepOutcome,
};

use super::fixtures::{Context, Destination, Dispatch, Parcel, Receipt, Report, Routing, dispatch};

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
fn public_state_remains_terminal_after_success_or_policy_error() {
    // Given: public operation state with distinct input, metadata, child-state and output types.
    for finish in [Finish::Success, Finish::SplitError, Finish::CombineError] {
        let reductions = Cell::new(0);
        let policy = Policy {
            finish,
            reductions: &reductions,
        };
        let mut hardware = FanOut::new(vec![Destination::new(0)], policy);
        let mut context = Context::default();
        let mut state: FanOutState<Dispatch, String, Parcel, Option<Parcel>, Receipt> =
            <FanOut<Destination, Policy<'_>> as Stage<Context>>::start(dispatch(&[0]));

        // When: direct stepping finishes the request and the caller steps the same state again.
        let first = hardware.step(&mut context, &mut state);
        let (result, expected, deliveries) = match finish {
            Finish::SplitError => (first, Err(BranchError::Application("split failed")), 0),
            Finish::Success | Finish::CombineError => {
                assert_eq!(first, StepOutcome::Progressed);
                assert_eq!(
                    hardware.step(&mut context, &mut state),
                    StepOutcome::Progressed
                );
                let expected = match finish {
                    Finish::Success => Ok(Report {
                        label: "delivery".into(),
                        receipts: vec![Receipt(0)],
                    }),
                    Finish::CombineError => Err(BranchError::Application("combine failed")),
                    Finish::SplitError => Err(BranchError::Application("split failed")),
                };
                (hardware.step(&mut context, &mut state), expected, 1)
            }
        };
        let repeated = hardware.step(&mut context, &mut state);

        // Then: neither planning, committed delivery nor reduction is replayed.
        assert_eq!(result, StepOutcome::Complete(expected));
        assert_eq!(repeated, StepOutcome::Exhausted);
        assert_eq!(context.plans.get(), 1);
        assert_eq!(context.attempts.len(), deliveries);
        assert_eq!(context.deliveries.len(), deliveries);
        assert_eq!(reductions.get(), deliveries);
    }
}
