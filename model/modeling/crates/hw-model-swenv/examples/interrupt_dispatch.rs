//! Advanced static ISR assembly, dispatch observation, and cooperative shutdown.
//!
//! Run: `cargo run -p hw-model-swenv --example interrupt_dispatch`.
//! Start with the README's scoped `run_main` example for software registration.
//! This lower-level alternative freezes handlers before spawning the dispatcher;
//! its owner must request shutdown and join to observe the final result.

/// Runtime-neutral toy source, handler, observer, and frozen registry.
pub mod support;

use hw_model_swenv::{InterruptDispatcher, SharedInterruptObserver, SoftwareEnv, TraceBuffer};
use std::{
    error::Error,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
};
use support::{Notice, Observer, Source, registry};

async fn run_happy() -> Result<(), Box<dyn Error>> {
    // Source and handlers log independently; the lifecycle observer adds trace
    // entries without making the hardware source own software handler objects.
    let log = Arc::new(Mutex::new(Vec::new()));
    let entries = Arc::new(Mutex::new(0usize));
    let trace = TraceBuffer::new();
    let independent = TraceBuffer::new();
    independent.push(());
    let source = Source::new([Notice(1), Notice(2)], Arc::clone(&log));
    let source_drops = Arc::clone(&source.drops);
    let handler_drops = Arc::new(AtomicUsize::new(0));
    let observer: SharedInterruptObserver<Notice> = Arc::new(Observer {
        log: Arc::clone(&log),
        trace: trace.clone(),
    });
    let task = InterruptDispatcher::start(
        Arc::new(SoftwareEnv::new()),
        registry(
            Arc::clone(&log),
            Arc::clone(&entries),
            Arc::clone(&handler_drops),
        )?,
        source.clone(),
        observer,
    );
    while source.consumed() != 2 {
        tokio::task::yield_now().await;
    }
    let summary = task.shutdown_and_join().await?;
    // Joining also accounts for an already accepted handler before releasing it.
    drop(source);
    assert_eq!(source_drops.load(Ordering::SeqCst), 2);
    assert_eq!(handler_drops.load(Ordering::SeqCst), 2);
    let order = log
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone();
    println!(
        "happy order={} summary={} trace={:?} independent={:?} source_drops={} handler_drops={}",
        order.join(","),
        summary.deliveries(),
        trace.snapshot(),
        independent.snapshot(),
        source_drops.load(Ordering::SeqCst),
        handler_drops.load(Ordering::SeqCst)
    );
    Ok(())
}

async fn run_adverse() -> Result<(), Box<dyn Error>> {
    // Vector 9 has no installed handler. The dispatcher returns an error rather
    // than silently consuming an interrupt software cannot service.
    let adverse_log = Arc::new(Mutex::new(Vec::new()));
    let adverse_entries = Arc::new(Mutex::new(0usize));
    let adverse_trace = TraceBuffer::new();
    let adverse_source = Source::new([Notice(9)], Arc::clone(&adverse_log));
    let adverse_source_drops = Arc::clone(&adverse_source.drops);
    let adverse_handler_drops = Arc::new(AtomicUsize::new(0));
    let adverse_observer: SharedInterruptObserver<Notice> = Arc::new(Observer {
        log: Arc::clone(&adverse_log),
        trace: adverse_trace.clone(),
    });
    let adverse = InterruptDispatcher::start(
        Arc::new(SoftwareEnv::new()),
        registry(
            Arc::clone(&adverse_log),
            Arc::clone(&adverse_entries),
            Arc::clone(&adverse_handler_drops),
        )?,
        adverse_source.clone(),
        adverse_observer,
    );
    while adverse_source.consumed() == 0 {
        tokio::task::yield_now().await;
    }
    let unregistered = adverse.shutdown_and_join().await.is_err();
    drop(adverse_source);
    // Separately, request shutdown before the spawned task accepts its first IRQ.
    let shutdown_source = Source::new([Notice(1)], Arc::new(Mutex::new(Vec::new())));
    let shutdown_source_drops = Arc::clone(&shutdown_source.drops);
    let shutdown_entries = Arc::new(Mutex::new(0usize));
    let shutdown_handler_drops = Arc::new(AtomicUsize::new(0));
    let shutdown = InterruptDispatcher::start(
        Arc::new(SoftwareEnv::new()),
        registry(
            Arc::new(Mutex::new(Vec::new())),
            Arc::clone(&shutdown_entries),
            Arc::clone(&shutdown_handler_drops),
        )?,
        shutdown_source.clone(),
        Arc::new(hw_model_swenv::NoopInterruptObserver),
    );
    shutdown.shutdown_handle().request_shutdown();
    let shutdown_summary = shutdown.shutdown_and_join().await?;
    let shutdown_consumed = shutdown_source.consumed();
    drop(shutdown_source);
    assert_eq!(adverse_source_drops.load(Ordering::SeqCst), 2);
    assert_eq!(adverse_handler_drops.load(Ordering::SeqCst), 2);
    assert_eq!(shutdown_source_drops.load(Ordering::SeqCst), 2);
    assert_eq!(shutdown_handler_drops.load(Ordering::SeqCst), 2);
    println!(
        "adverse unregistered={unregistered} env_entries={} lifecycle_trace={} shutdown_deliveries={} shutdown_consumed={} source_drops={} handler_drops={} shutdown_source_drops={} shutdown_handler_drops={}",
        *adverse_entries
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner),
        adverse_trace.snapshot().len(),
        shutdown_summary.deliveries(),
        shutdown_consumed,
        adverse_source_drops.load(Ordering::SeqCst),
        adverse_handler_drops.load(Ordering::SeqCst),
        shutdown_source_drops.load(Ordering::SeqCst),
        shutdown_handler_drops.load(Ordering::SeqCst)
    );
    Ok(())
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn Error>> {
    run_happy().await?;
    run_adverse().await
}
