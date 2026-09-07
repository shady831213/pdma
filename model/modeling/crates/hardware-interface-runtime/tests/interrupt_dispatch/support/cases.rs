use super::{Notice, Observer, RecordingHandler, ToyError, ToySource, freeze, noop, registry};
use hardware_interface_runtime::{
    HandlerRegistry, InterruptDispatcher, InterruptHandler, SharedInterruptObserver, SoftwareCpu,
    TraceBuffer,
};
use std::sync::{Arc, Mutex};

struct StatefulHandler {
    turns: usize,
    observed: Arc<Mutex<Vec<usize>>>,
}

impl InterruptHandler<Notice> for StatefulHandler {
    type Error = ToyError;

    async fn handle(&mut self, _notification: &Notice) -> Result<(), Self::Error> {
        self.turns = self.turns.saturating_add(1);
        self.observed
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(self.turns);
        Ok(())
    }
}

#[tokio::test(flavor = "current_thread")]
async fn shared_vector_reuses_one_persistent_handler_state() {
    let observed = Arc::new(Mutex::new(Vec::new()));
    let mut handlers = HandlerRegistry::new();
    assert!(
        handlers
            .register(
                1,
                StatefulHandler {
                    turns: 0,
                    observed: Arc::clone(&observed)
                }
            )
            .is_ok()
    );
    let source = ToySource::new([
        Ok(Some(Notice {
            vector: 1,
            value: 1,
        })),
        Ok(Some(Notice {
            vector: 1,
            value: 2,
        })),
    ]);
    let task = InterruptDispatcher::start(
        Arc::new(SoftwareCpu::new()),
        freeze(handlers),
        source,
        noop(),
    );
    while observed
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .len()
        != 2
    {
        tokio::task::yield_now().await;
    }
    let summary = task.shutdown_and_join().await.expect("joins");
    assert_eq!(summary.deliveries(), 2);
    assert_eq!(
        *observed
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner),
        [1, 2]
    );
}

#[tokio::test(flavor = "current_thread")]
async fn duplicate_registration_preserves_both_handlers_and_empty_freeze_is_recoverable() {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let original = RecordingHandler {
        calls: Arc::clone(&calls),
        release: None,
        fail: false,
    };
    let replacement = RecordingHandler {
        calls,
        release: None,
        fail: false,
    };
    let mut registry = HandlerRegistry::new();
    assert!(registry.register(1, original).is_ok());
    let duplicate = registry.register(1, replacement).expect_err("duplicate");
    assert_eq!(*duplicate.vector(), 1);
    let _replacement = duplicate.into_handler();
    let source = ToySource::new([Ok(Some(Notice {
        vector: 1,
        value: 11,
    }))]);
    let task = InterruptDispatcher::start(
        Arc::new(SoftwareCpu::new()),
        freeze(registry),
        source.clone(),
        noop(),
    );
    while source.consumed() == 0 {
        tokio::task::yield_now().await;
    }
    assert_eq!(
        task.shutdown_and_join().await.expect("joins").deliveries(),
        1
    );
    let empty = HandlerRegistry::<u8, Notice, RecordingHandler>::new();
    let recovered = empty
        .freeze()
        .expect_err("empty registry cannot freeze")
        .into_registry();
    assert!(recovered.is_empty());
}

#[tokio::test(flavor = "current_thread")]
async fn transient_none_then_success_uses_persistent_handler_and_observer_order() {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let source = ToySource::new([
        Ok(None),
        Ok(Some(Notice {
            vector: 1,
            value: 7,
        })),
    ]);
    let observer = Arc::new(Observer::default());
    let shared: SharedInterruptObserver<Notice> = observer.clone();
    let task = InterruptDispatcher::start(
        Arc::new(SoftwareCpu::new()),
        registry(RecordingHandler {
            calls: Arc::clone(&calls),
            release: None,
            fail: false,
        }),
        source.clone(),
        shared,
    );
    while source.consumed() == 0 {
        tokio::task::yield_now().await;
    }
    let summary = task.shutdown_and_join().await.expect("joins");
    assert_eq!(summary.deliveries(), 1);
    assert_eq!(
        *calls
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner),
        [7]
    );
    assert_eq!(
        *observer
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner),
        ["start", "success"]
    );
}

#[tokio::test(flavor = "current_thread")]
async fn shutdown_during_claim_consumes_nothing() {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let source = ToySource::new([Ok(Some(Notice {
        vector: 1,
        value: 3,
    }))]);
    source.lock().gate_claim = true;
    let task = InterruptDispatcher::start(
        Arc::new(SoftwareCpu::new()),
        registry(RecordingHandler {
            calls,
            release: None,
            fail: false,
        }),
        source.clone(),
        noop(),
    );
    while source.lock().claim_calls == 0 {
        tokio::task::yield_now().await;
    }
    let summary = task.shutdown_and_join().await.expect("joins");
    assert_eq!(summary.deliveries(), 0);
    assert_eq!(source.consumed(), 0);
}

#[tokio::test(flavor = "current_thread")]
async fn shutdown_already_requested_wins_over_ready_source() {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let source = ToySource::new([Ok(Some(Notice {
        vector: 1,
        value: 4,
    }))]);
    let task = InterruptDispatcher::start(
        Arc::new(SoftwareCpu::new()),
        registry(RecordingHandler {
            calls,
            release: None,
            fail: false,
        }),
        source.clone(),
        noop(),
    );
    task.shutdown_handle().request_shutdown();
    let summary = task.shutdown_and_join().await.expect("joins");
    assert_eq!(summary.deliveries(), 0);
    assert_eq!(source.consumed(), 0);
}

#[test]
fn trace_buffers_are_independent_clone_shared_saturating_and_clear_resets() {
    let first = TraceBuffer::new();
    let clone = first.clone();
    let second = TraceBuffer::new();
    first.push_with(|sequence| sequence);
    clone.push_with(|sequence| sequence);
    second.push_with(|sequence| sequence);
    assert_eq!(first.events(), [0, 1]);
    assert_eq!(second.events(), [0]);
    clone.clear();
    first.push_with(|sequence| sequence);
    assert_eq!(first.events(), [0]);
}
