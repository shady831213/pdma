use crate::{Child, RejectPlan};
use hardware_interface_pipeline::{BranchError, Fan, RunError, Stage, StepOutcome};

#[test]
fn first_domain_error_survives_settlement_failures_and_preserves_replacement_offer() {
    // Given: planning will fail while one source still owns accepted work.
    let mut graph = Fan::new(
        vec![
            Child {
                immediate: true,
                ..Child::default()
            },
            Child {
                raw_on_finish: true,
                finish_error: Some(23),
                ..Child::default()
            },
        ],
        RejectPlan,
        Vec::<Child>::new(),
    );
    let mut effects = Vec::new();
    let mut offer = Some(vec![1, 7]);
    assert_eq!(
        graph.step(&mut effects, &mut offer),
        Ok(StepOutcome::InProgress)
    );
    assert_eq!(
        graph.step(&mut effects, &mut offer),
        Ok(StepOutcome::InProgress)
    );
    offer = Some(vec![8, 9]);
    // When: the accepted peer reports a raw failure followed by a second domain error.
    assert_eq!(
        graph.step(&mut effects, &mut offer),
        Err(RunError::BudgetExhausted)
    );
    assert_eq!(offer, Some(vec![8, 9]));
    let result = graph.step(&mut effects, &mut offer);
    // Then: the first error survives, the peer finishes, and the new offer is untouched.
    assert_eq!(
        result,
        Ok(StepOutcome::Complete(Err(BranchError::Application(42))))
    );
    assert_eq!(effects, [1, 7]);
    assert_eq!(offer, Some(vec![8, 9]));
}
