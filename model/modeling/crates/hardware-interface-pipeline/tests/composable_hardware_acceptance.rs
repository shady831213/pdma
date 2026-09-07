#![allow(missing_docs, reason = "external behavior acceptance tests")]

pub mod composable_hardware_acceptance {
    pub mod backend;
    pub mod stages;
}

use std::{future::Future, task::Context};

use composable_hardware_acceptance::{backend::Backend, stages::*};
use hardware_interface_pipeline::{AsyncExecutor, Operation, PipelineExt, StepOutcome};

#[test]
fn nested_hardware_reuses_objects_and_propagates_domain_results() {
    // Given existing hardware and non-Clone requests with correlation identity.
    let mut hardware = Decode::default().then(
        Compute {
            factor: 2,
            executed: 0,
        }
        .then(Publish),
    );
    let mut environment = Environment {
        writable: true,
        ..Environment::default()
    };
    for (request, expected) in [
        (
            Request {
                id: 41,
                bytes: vec![3, 7],
            },
            Response {
                id: 41,
                result: Ok(20),
            },
        ),
        (
            Request {
                id: 42,
                bytes: vec![200],
            },
            Response {
                id: 42,
                result: Err(Overflow),
            },
        ),
    ] {
        // When a fresh operation traverses the same nested hardware topology.
        let mut operation = Operation::new(request);
        assert_eq!(
            operation.step(&mut hardware, &mut environment),
            StepOutcome::Progressed
        );
        assert_eq!(
            operation.step(&mut hardware, &mut environment),
            StepOutcome::Progressed
        );
        // Then faults are data delivered to publication, and completion is inert.
        assert_eq!(
            operation.step(&mut hardware, &mut environment),
            StepOutcome::Complete(Ok(expected))
        );
        assert_eq!(
            operation.step(&mut hardware, &mut environment),
            StepOutcome::Exhausted
        );
    }
    assert_eq!(hardware.upstream.accepted, 2);
    assert_eq!(hardware.downstream.upstream.executed, 2);
    assert_eq!(environment.responses, [41, 42]);
}

#[tokio::test]
async fn dropping_native_driver_future_preserves_pending_transfer() {
    // Given a full response sink after execution has consumed an owned request.
    let backend = Backend::default();
    let executor = AsyncExecutor::new(&backend);
    let mut hardware = Decode::default().then(
        Compute {
            factor: 3,
            executed: 0,
        }
        .then(Publish),
    );
    let mut operation = Operation::new(Request {
        id: 9,
        bytes: vec![2, 5],
    });
    let mut future = Box::pin(executor.run(&mut hardware, &mut operation));
    assert!(
        future
            .as_mut()
            .poll(&mut Context::from_waker(std::task::Waker::noop()))
            .is_pending()
    );
    // When only the standard driving future is canceled, then resumed after readiness.
    drop(future);
    assert_eq!(hardware.upstream.accepted, 1);
    assert_eq!(hardware.downstream.upstream.executed, 1);
    assert!(backend.published().unwrap().is_empty());
    backend.open().unwrap();
    let response = executor.run(&mut hardware, &mut operation).await.unwrap();
    // Then the retained value arrives once without re-execution or lost identity.
    assert_eq!(
        response,
        Response {
            id: 9,
            result: Ok(21)
        }
    );
    assert_eq!(hardware.downstream.upstream.executed, 1);
    assert_eq!(backend.published().unwrap(), [9]);
}

#[tokio::test]
async fn native_waker_resumes_backpressured_composition() {
    // Given a composed operation awaiting response capacity.
    let backend = Backend::default();
    let executor = AsyncExecutor::new(&backend);
    let mut hardware = Decode::default().then(
        Compute {
            factor: 4,
            executed: 0,
        }
        .then(Publish),
    );
    let mut operation = Operation::new(Request {
        id: 88,
        bytes: vec![1, 2],
    });
    // When another async actor opens the sink and wakes its registered waiter.
    let (response, ()) = tokio::join!(biased; executor.run(&mut hardware, &mut operation), async {
        backend.open().unwrap();
    });
    // Then ordinary async execution propagates the typed transformed response.
    assert_eq!(
        response.unwrap(),
        Response {
            id: 88,
            result: Ok(12)
        }
    );
    assert_eq!(backend.published().unwrap(), [88]);
    assert!(backend.waits() > 0);
}
