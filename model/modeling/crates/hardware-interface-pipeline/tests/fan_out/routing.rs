use hardware_interface_pipeline::{FanOut, Operation, RunOutcome};

use super::{
    fixtures::{Context, Destination, Parcel, Receipt, Report, Routing, dispatch},
    policies::{Broadcast, Split},
};

#[test]
fn dispatches_only_selected_destination_and_transforms_its_output() {
    // Given: an owned request selecting only the middle destination.
    let mut context = Context::default();
    let mut stage = FanOut::new((0..3).map(Destination::new).collect(), Routing);
    let mut operation = Operation::new(dispatch(&[1]));

    // When: splitting, delivery, and reduction each advance once.
    assert_eq!(
        operation.step(&mut stage, &mut context),
        Ok(RunOutcome::Progressed)
    );
    assert!(context.attempts.is_empty());
    assert_eq!(
        operation.step(&mut stage, &mut context),
        Ok(RunOutcome::Progressed)
    );
    let result = operation.step(&mut stage, &mut context);

    // Then: only that destination receives input and the application transforms its receipt.
    assert_eq!(
        result,
        Ok(RunOutcome::Complete(Ok(Report {
            label: "delivery".into(),
            receipts: vec![Receipt(1)],
        })))
    );
    assert_eq!(context.deliveries, [(1, "payload 1".into())]);
    assert_eq!(context.attempts, [1]);
    assert_eq!(context.plans.get(), 1);
}

#[test]
fn dispatches_subset_in_application_order_one_destination_per_frame() {
    // Given: the application selects a reversed subset of the available ports.
    let mut context = Context::default();
    let mut stage = FanOut::new((0..4).map(Destination::new).collect(), Routing);
    let mut operation = Operation::new(dispatch(&[3, 1]));

    // When: each frame advances one destination.
    assert_eq!(
        operation.step(&mut stage, &mut context),
        Ok(RunOutcome::Progressed)
    );
    let first = operation.step(&mut stage, &mut context);
    let first_deliveries = context.deliveries.clone();
    let second = operation.step(&mut stage, &mut context);
    let result = operation.step(&mut stage, &mut context);

    // Then: the order and output receipts follow the plan, excluding unselected ports.
    assert_eq!(first, Ok(RunOutcome::Progressed));
    assert_eq!(second, Ok(RunOutcome::Progressed));
    assert_eq!(first_deliveries, [(3, "payload 3".into())]);
    assert_eq!(context.attempts, [3, 1]);
    assert_eq!(
        result,
        Ok(RunOutcome::Complete(Ok(Report {
            label: "delivery".into(),
            receipts: vec![Receipt(3), Receipt(1)],
        })))
    );
}

#[test]
fn splits_non_clone_owned_input_and_combines_into_unrelated_output() {
    // Given: a non-Clone parcel whose ownership is split by the application.
    let mut context = Context::default();
    let mut stage = FanOut::new(vec![Destination::new(0), Destination::new(1)], Split);
    let mut operation = Operation::new(Parcel("abcdefg".into()));

    // When: the two split parts run.
    assert_eq!(
        operation.step(&mut stage, &mut context),
        Ok(RunOutcome::Progressed)
    );
    let first = operation.step(&mut stage, &mut context);
    let second = operation.step(&mut stage, &mut context);
    let result = operation.step(&mut stage, &mut context);

    // Then: different payloads reach each destination and receipts become a scalar.
    assert_eq!(first, Ok(RunOutcome::Progressed));
    assert_eq!(second, Ok(RunOutcome::Progressed));
    assert_eq!(result, Ok(RunOutcome::Complete(Ok(14))));
    assert_eq!(context.deliveries, [(1, "abc".into()), (0, "defg".into())]);
}

#[test]
fn broadcasts_using_application_defined_cloning() {
    // Given: the application explicitly clones its String for three destinations.
    let mut context = Context::default();
    let mut stage = FanOut::new((0..3).map(Destination::new).collect(), Broadcast(3));
    let mut operation = Operation::new("shared message".into());

    // When: splitting, three deliveries, and reduction run.
    let outcomes: Vec<_> = (0..5)
        .map(|_| operation.step(&mut stage, &mut context))
        .collect();

    // Then: each destination receives the full message exactly once.
    assert_eq!(
        outcomes,
        [
            Ok(RunOutcome::Progressed),
            Ok(RunOutcome::Progressed),
            Ok(RunOutcome::Progressed),
            Ok(RunOutcome::Progressed),
            Ok(RunOutcome::Complete(Ok(vec![
                Receipt(0),
                Receipt(1),
                Receipt(2)
            ])))
        ]
    );
    assert_eq!(
        context.deliveries,
        [
            (0, "shared message".into()),
            (1, "shared message".into()),
            (2, "shared message".into()),
        ]
    );
}

#[test]
fn empty_plan_combines_without_touching_destinations() {
    // Given: an empty delivery plan and retained application metadata.
    let mut context = Context::default();
    let mut stage = FanOut::<Destination, _>::new(vec![], Routing);
    let mut operation = Operation::new(dispatch(&[]));

    // When: the empty fan-out is advanced.
    assert_eq!(
        operation.step(&mut stage, &mut context),
        Ok(RunOutcome::Progressed)
    );
    assert_eq!(
        operation.step(&mut stage, &mut context),
        Ok(RunOutcome::Progressed)
    );
    let result = operation.step(&mut stage, &mut context);

    // Then: the application's empty receipt result retains its metadata.
    assert_eq!(
        result,
        Ok(RunOutcome::Complete(Ok(Report {
            label: "delivery".into(),
            receipts: vec![],
        })))
    );
    assert!(context.attempts.is_empty());
    assert_eq!(context.plans.get(), 1);
}
