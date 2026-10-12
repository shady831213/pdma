use hardware_interface_pipeline::{BranchError, Dispatch, Operation, Stage, StepOutcome, Then};

use super::fixtures::{Context, Destination, Routing, request};

struct CountDeliveries;

impl Stage<Context> for CountDeliveries {
    type Input = ();
    type Output = usize;
    type Error = BranchError<&'static str>;
    type Wait = &'static str;
    type State = ();

    fn start((): ()) {}

    fn step(
        &mut self,
        context: &mut Context,
        (): &mut (),
    ) -> StepOutcome<usize, Self::Error, Self::Wait> {
        StepOutcome::Complete(Ok(context.deliveries.len()))
    }
}

#[test]
fn then_consumes_unit_after_dispatch_reduction() {
    // Given: Dispatch feeds its unit output into an ordinary downstream stage.
    let mut context = Context::default();
    let mut hardware = Then::new(
        Dispatch::new(vec![Destination::new(0)], Routing::Accept),
        CountDeliveries,
    );
    let mut operation = Operation::new(request(0));

    // When: planning, delivery, reduction, and downstream execution advance.
    let outcomes: Vec<_> = (0..4)
        .map(|_| operation.step(&mut hardware, &mut context))
        .collect();

    // Then: downstream receives unit only after publication is complete.
    assert_eq!(
        outcomes,
        [
            StepOutcome::Progressed,
            StepOutcome::Progressed,
            StepOutcome::Progressed,
            StepOutcome::Complete(Ok(1))
        ]
    );
    assert_eq!(context.deliveries, [(0, "routed owned payload".into())]);
    assert_eq!(context.routes.get(), 1);
}
