use super::fixtures::{Context, Destination, Routing, Source, finish, request};
use hardware_interface_pipeline::{
    BranchError, Dispatch, FanOut, RunError, Stage, StepOutcome, StepResult, Then,
};
struct Count;
impl Stage<Context> for Count {
    type Input = Vec<(usize, ())>;
    type Output = usize;
    type Error = BranchError<&'static str>;
    type State = Option<Self::Input>;
    fn start(input: Self::Input) -> Self::State {
        Some(input)
    }
    fn step(&mut self, _: &mut Context, state: &mut Self::State) -> StepResult<usize, Self::Error> {
        Ok(StepOutcome::Complete(Ok(state
            .take()
            .ok_or(RunError::Exhausted)?
            .len())))
    }
}
#[test]
fn then_consumes_dispatch_receipts_after_a_separate_boundary() {
    // Given: a routed fan followed by an application receipt reducer.
    let fan = FanOut::new(
        Source,
        Dispatch::new(Routing::Accept),
        vec![Destination::new(0)],
    );
    let mut hardware = Then::new(fan, Count);
    let mut state = <Then<FanOut<Source, Dispatch<Routing>, Destination>, Count> as Stage<
        Context,
    >>::start(request(0));
    let mut context = Context::default();
    // When: delivery completes before downstream reduction.
    for _ in 0..3 {
        assert_eq!(
            hardware.step(&mut context, &mut state),
            Ok(StepOutcome::InProgress)
        );
    }
    assert_eq!(context.deliveries.len(), 1);
    let result = finish(&mut hardware, &mut state, &mut context);
    // Then: the reducer receives exactly the committed receipt.
    assert_eq!(result, Ok(StepOutcome::Complete(Ok(1))));
}
