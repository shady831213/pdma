//! Standard error propagation preserves each pipeline failure's typed origin.

use hw_model_pipeline::{AsyncRunError, BranchError, RunError};
use std::{error::Error, io};

#[test]
fn branch_failure_preserves_the_application_source_when_propagated() {
    // Given an application failure routed through a branch.
    let failure = BranchError::Application(io::Error::from(io::ErrorKind::PermissionDenied));
    // When an ordinary library consumer propagates it through the standard error interface.
    let error: Box<dyn Error> = failure.into();
    // Then its source remains available for typed diagnosis.
    let source = error.source().unwrap().downcast_ref::<io::Error>().unwrap();
    assert_eq!(source.kind(), io::ErrorKind::PermissionDenied);
}

#[test]
fn routing_failure_has_no_invented_application_source() {
    // Given an invalid routing plan, without an application failure.
    let failure = BranchError::<io::Error>::InvalidIndex { index: 3, ports: 2 };
    // When reported through standard error handling.
    let error: Box<dyn Error> = failure.into();
    // Then it has a usable diagnostic but no nested source.
    assert!(!error.to_string().is_empty());
    assert!(error.source().is_none());
}

#[test]
fn async_failures_preserve_each_origin_in_the_source_chain() {
    // Given failures from each asynchronous execution boundary.
    let failures: [AsyncRunError<io::Error, io::Error>; 3] = [
        AsyncRunError::BackendInteraction(io::Error::from(io::ErrorKind::PermissionDenied)),
        AsyncRunError::Stage(io::Error::from(io::ErrorKind::InvalidData)),
        AsyncRunError::Run(RunError::BudgetExhausted),
    ];
    // When the caller propagates them through a common standard error boundary.
    let errors = failures.map(|failure| -> Box<dyn Error> { failure.into() });
    // Then original typed causes remain distinguishable.
    let [backend, stage, run] = errors;
    assert_eq!(
        backend
            .source()
            .unwrap()
            .downcast_ref::<io::Error>()
            .unwrap()
            .kind(),
        io::ErrorKind::PermissionDenied
    );
    assert_eq!(
        stage
            .source()
            .unwrap()
            .downcast_ref::<io::Error>()
            .unwrap()
            .kind(),
        io::ErrorKind::InvalidData
    );
    assert_eq!(
        run.source().unwrap().downcast_ref::<RunError>(),
        Some(&RunError::BudgetExhausted)
    );
}
