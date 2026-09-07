use std::{
    future::Future,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    task::{Context, Poll, Waker},
};

use hardware_interface_pipeline::AsyncInteractionBackend;

use super::{DeviceInteractionBackend, DeviceWait};
use crate::runtime::state::RunnerState;

mod updates;

fn backend() -> Arc<DeviceInteractionBackend> {
    Arc::new(DeviceInteractionBackend::new(RunnerState::new(
        crate::DeviceCore::new(
            Arc::new(hardware_interface_runtime::SharedMemory::new()),
            pdma_model_contract::semantic::ResourceConfig::default(),
        ),
    )))
}

fn assert_pipeline_backend<B: AsyncInteractionBackend<RunnerState, DeviceWait>>() {}

#[test]
fn interaction_backend_implements_pipeline_contract() {
    assert_pipeline_backend::<DeviceInteractionBackend>();
}

#[tokio::test]
async fn interaction_backend_cancellation_has_exact_commit_boundaries() {
    let backend = backend();
    let closures = Arc::new(AtomicUsize::new(0));
    let (ready_sender, ready_receiver) = std::sync::mpsc::channel();
    let (release_sender, release_receiver) = std::sync::mpsc::channel();
    let held_backend = Arc::clone(&backend);
    let held = std::thread::spawn(move || {
        tokio::runtime::Builder::new_current_thread()
            .build()
            .expect("holder runtime")
            .block_on(held_backend.hold_for_test(ready_sender, release_receiver));
    });
    ready_receiver.recv().expect("backend holder starts");

    let closure_counter = Arc::clone(&closures);
    let mut cancelled = Box::pin(backend.interact(move |state| {
        closure_counter.fetch_add(1, Ordering::SeqCst);
        state.generation = state.generation.saturating_add(1);
    }));
    let waker = Waker::noop();
    let mut context = Context::from_waker(waker);
    assert!(matches!(
        cancelled.as_mut().poll(&mut context),
        Poll::Pending
    ));
    drop(cancelled);
    release_sender.send(()).expect("release holder");
    held.join().expect("holder joins");
    assert_eq!(closures.load(Ordering::SeqCst), 0);
    assert_eq!(backend.interact(|state| state.generation).await, 0);

    let closure_counter = Arc::clone(&closures);
    let committed = backend.interact(move |state| {
        closure_counter.fetch_add(1, Ordering::SeqCst);
        state.generation = state.generation.saturating_add(1);
    });
    committed.await;
    assert_eq!(closures.load(Ordering::SeqCst), 1);
    assert_eq!(backend.interact(|state| state.generation).await, 1);
    println!("pre_lock_closures=0 post_closure_commits=1 generation=1");
}

#[tokio::test]
async fn interaction_backend_wait_rechecks_after_subscribe() {
    let backend = backend();
    let wait = DeviceWait::generation(0);
    backend
        .interact(|state| state.generation = state.generation.saturating_add(1))
        .await;
    backend.publish(1);
    backend.wait_for_change(wait).await;
    assert_eq!(backend.interact(|state| state.generation).await, 1);
    println!("subscribed_recheck_generation=1 wake_observed=true");
}
