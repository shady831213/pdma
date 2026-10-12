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
impl Stage<usize> for Source {
    type Input = Value;
    type Output = Value;
    type Error = ();
    type State = Option<Value>;
    fn start(input: Value) -> Self::State {
        Some(input)
    }
    fn step(&mut self, _: &mut usize, state: &mut Self::State) -> StepResult<Value, ()> {
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
impl Stage<usize> for Receipt {
    type Input = Value;
    type Output = Value;
    type Error = ();
    type State = Option<Value>;
    fn start(input: Value) -> Self::State {
        Some(input)
    }
    fn step(&mut self, writes: &mut usize, state: &mut Self::State) -> StepResult<Value, ()> {
        *writes += 1;
        Ok(StepOutcome::Complete(Ok(state
            .take()
            .ok_or(RunError::Exhausted)?)))
    }
}

#[test]
fn reset_drops_retained_receipts_and_unselected_values_once() {
    // Given: non-Clone owned values with observable resource disposal.
    let drops = Arc::new(AtomicUsize::new(0));
    let mut hardware = FanIn::new(vec![Source, Source], Arbitrate::new(Select), Receipt);
    let mut operation = Operation::new(vec![Value(Arc::clone(&drops)), Value(Arc::clone(&drops))]);
    let mut writes = 0;
    for _ in 0..3 {
        assert!(matches!(
            operation.step(&mut hardware, &mut writes),
            Ok(StepOutcome::InProgress)
        ));
    }
    assert_eq!(writes, 1);
    assert_eq!(drops.load(Ordering::SeqCst), 0);
    // When: explicit cancellation discards a receipt and an unselected buffered value.
    operation.reset();
    // Then: both resources are released once and committed effects remain.
    assert_eq!(drops.load(Ordering::SeqCst), 2);
    assert_eq!(writes, 1);
    assert!(matches!(
        operation.step(&mut hardware, &mut writes),
        Err(RunError::Exhausted)
    ));
}

#[test]
fn completion_transfers_values_and_leaves_no_invocation_resource() {
    // Given: a finite batch with one owned resource.
    let drops = Arc::new(AtomicUsize::new(0));
    let mut hardware = FanIn::new(vec![Source], Arbitrate::new(Select), Receipt);
    let mut state =
        <FanIn<Source, Arbitrate<Select>, Receipt> as Stage<usize>>::start(vec![Value(
            Arc::clone(&drops),
        )]);
    let mut writes = 0;
    for _ in 0..2 {
        assert!(matches!(
            hardware.step(&mut writes, &mut state),
            Ok(StepOutcome::InProgress)
        ));
    }
    // When: completion transfers the sole owned receipt to the caller.
    let result = hardware.step(&mut writes, &mut state);
    // Then: child state is gone, and only the returned result retains the resource.
    assert!(state.is_none());
    assert_eq!(drops.load(Ordering::SeqCst), 0);
    assert!(matches!(result, Ok(StepOutcome::Complete(Ok(_)))));
    drop(result);
    assert_eq!(drops.load(Ordering::SeqCst), 1);
    assert_eq!(writes, 1);
}
