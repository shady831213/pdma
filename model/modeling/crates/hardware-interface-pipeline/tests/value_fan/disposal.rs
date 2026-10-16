use hardware_interface_pipeline::{
    Arbitrate, Arbitration, FanIn, Operation, RunError, Stage, StepOutcome, StepResult,
};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

struct Value(Arc<AtomicUsize>);
impl Drop for Value {
    fn drop(&mut self) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}
struct Source;
#[derive(Default)]
struct Context {
    writes: usize,
    reset: bool,
}
impl Stage<Context> for Source {
    type Input = Value;
    type Output = Value;
    type Error = ();
    fn step(&mut self, _: &mut Context, state: &mut Option<Self::Input>) -> StepResult<Value, ()> {
        Ok(StepOutcome::Complete(Ok(state
            .take()
            .ok_or(RunError::Exhausted)?)))
    }
}
struct Select;
impl Arbitration<Value, ()> for Select {
    fn select(&mut self, values: &[Option<Value>]) -> Result<Option<usize>, ()> {
        Ok(values.iter().position(Option::is_some))
    }
}
struct Receipt;
impl Stage<Context> for Receipt {
    type Input = Value;
    type Output = Value;
    type Error = ();
    fn step(
        &mut self,
        context: &mut Context,
        state: &mut Option<Self::Input>,
    ) -> StepResult<Value, ()> {
        if context.reset {
            state.take();
            return Ok(StepOutcome::Complete(Err(())));
        }
        context.writes += 1;
        Ok(StepOutcome::Complete(Ok(state
            .take()
            .ok_or(RunError::Exhausted)?)))
    }
}

#[test]
fn stage_reset_drops_retained_receipts_and_buffered_values_once() {
    // Given: non-Clone owned values with observable resource disposal.
    let drops = Arc::new(AtomicUsize::new(0));
    let mut hardware = FanIn::new(vec![Source, Source], Arbitrate::new(Select), Receipt);
    let mut operation = Operation::new(vec![Value(Arc::clone(&drops)), Value(Arc::clone(&drops))]);
    let mut context = Context::default();
    for _ in 0..3 {
        assert!(matches!(
            operation.step(&mut hardware, &mut context),
            Ok(StepOutcome::InProgress)
        ));
    }
    assert_eq!(context.writes, 1);
    assert_eq!(drops.load(Ordering::SeqCst), 0);
    // When: the next receipt stage observes reset and completes with an error.
    context.reset = true;
    for _ in 0..8 {
        if let Ok(StepOutcome::Complete(result)) = operation.step(&mut hardware, &mut context) {
            assert_eq!(
                result.err(),
                Some(hardware_interface_pipeline::BranchError::Application(()))
            );
            break;
        }
    }
    assert!(operation.is_complete());
    // Then: both resources are released once and committed effects remain.
    assert_eq!(drops.load(Ordering::SeqCst), 2);
    assert_eq!(context.writes, 1);
    assert!(matches!(
        operation.step(&mut hardware, &mut context),
        Err(RunError::Exhausted)
    ));
}

#[test]
fn completion_transfers_values_and_leaves_no_invocation_resource() {
    // Given: a finite batch with one owned resource.
    let drops = Arc::new(AtomicUsize::new(0));
    let mut hardware = FanIn::new(vec![Source], Arbitrate::new(Select), Receipt);
    let mut state = hardware_interface_pipeline::Operation::new(vec![Value(Arc::clone(&drops))]);
    let mut context = Context::default();
    for _ in 0..2 {
        assert!(matches!(
            state.step(&mut hardware, &mut context),
            Ok(StepOutcome::InProgress)
        ));
    }
    // When: completion transfers the sole owned receipt to the caller.
    let result = state.step(&mut hardware, &mut context);
    // Then: child state is gone, and only the returned result retains the resource.
    assert!(state.is_complete());
    assert_eq!(drops.load(Ordering::SeqCst), 0);
    assert!(matches!(result, Ok(StepOutcome::Complete(Ok(_)))));
    drop(result);
    assert_eq!(drops.load(Ordering::SeqCst), 1);
    assert_eq!(context.writes, 1);
}
