//! Runtime-backed PDMA interrupt delivery surface.

#[path = "irq_runtime_surface/support.rs"]
pub mod support;

use hardware_interface_runtime::{HandlerRegistry, InterruptLifecycleObserver, SoftwareCpu};
use pdma_model_contract::semantic::{IrqVector, TraceEventKind};
use pdma_model_host::AsyncDriver;
use pdma_model_host::runtime::interrupt::{
    PdmaInterruptSource, PdmaInterruptTrace, PdmaIrqDelivery, PdmaIrqService,
};
use std::sync::Arc;
use support::{Facts, PersistentHandler, ProbeAccess, ProbePort, lock};

#[tokio::test(flavor = "current_thread")]
async fn task23_irq_runtime_surface() {
    let cpu = Arc::new(SoftwareCpu::new());
    let port = ProbePort::success();
    let mut setup_port = port.clone();
    let driver = cpu
        .run_main(AsyncDriver::setup_on(&mut setup_port, 4, 4))
        .await
        .expect("active setup");
    let vector = IrqVector::new(64);
    let access = ProbeAccess::success(vector, driver.queue_config_handle());
    let facts = Arc::new(std::sync::Mutex::new(Facts::default()));
    let handler = PersistentHandler::new(Arc::clone(&facts));
    let service = PdmaIrqService::new(driver, port.clone(), handler);
    let mut registry = HandlerRegistry::new();
    assert!(registry.register(vector, service).is_ok());
    let registry = registry.freeze().unwrap_or_else(|_| std::process::abort());
    let trace = Arc::new(PdmaInterruptTrace::default());
    let observer: Arc<dyn InterruptLifecycleObserver<_>> = trace.clone();
    let task = PdmaIrqDelivery::start(
        Arc::clone(&cpu),
        registry,
        PdmaInterruptSource::new(access.clone()),
        observer,
    );
    while lock(&facts).turns != 3 {
        tokio::task::yield_now().await;
    }
    let summary = task.shutdown_and_join().await.expect("cooperative join");
    let cpu_released = cpu.run_main(async { true }).await;
    let lifecycle_events = trace.events();
    let fact_counts = {
        let state = lock(&facts);
        (state.completions, state.events, state.turns)
    };
    let port_state = port.snapshot();
    let source_state = access.snapshot();
    let claims = source_state.notifications;
    let empty = source_state.empty;
    let handlers = fact_counts.2;
    let completions = fact_counts.0;
    let events = fact_counts.1;
    let acks = port_state.acks;
    let observer = lifecycle_events
        .iter()
        .map(|event| match (event.kind(), event.detail()) {
            (TraceEventKind::IrqHandlerStarted, _) => "start",
            (TraceEventKind::IrqHandlerCompleted, "Success") => "success",
            (TraceEventKind::IrqHandlerCompleted, "Error") => "error",
            _ => "unexpected",
        })
        .collect::<Vec<_>>()
        .join(",");
    let persistent_turns = fact_counts.2;
    let joined = summary.deliveries() == 3;
    assert_eq!(summary.deliveries(), 3);
    assert_eq!(fact_counts, (2, 1, 3));
    assert_eq!(port_state.acks, 3);
    assert_eq!(source_state.notifications, 3);
    assert_eq!(source_state.empty, 1);
    assert_eq!(source_state.transcript_lengths, vec![11, 2, 11, 11]);
    assert_eq!(lifecycle_events.len(), 6);
    assert!(cpu_released);

    let adverse = support::run_adverse().await;
    assert!(adverse.all(), "{adverse:?}");
    let residual_tasks = usize::from(
        !adverse.accepted_cpu_wait_completed() || !adverse.pending_handler_completed() || !joined,
    );
    let manual_irq_methods = support::manual_irq_methods();
    assert_eq!(residual_tasks, 0);
    assert_eq!(manual_irq_methods, 0);
    println!(
        "claims={claims};empty={empty};handlers={handlers};completions={completions};events={events};acks={acks};observer={observer};persistent_turns={persistent_turns};cpu_released={cpu_released};joined={joined}"
    );
    println!(
        "stale_before_drain={};unregistered={};ready_error={};claim_error={};shutdown_bias={};precommit_cancelled={};accepted_cpu_wait_completed={};pending_handler_completed={};drain_error_no_ack={};handler_error_no_ack={};ack_error={};residual_tasks={residual_tasks};manual_irq_methods={manual_irq_methods}",
        adverse.stale_before_drain(),
        adverse.unregistered(),
        adverse.ready_error(),
        adverse.claim_error(),
        adverse.shutdown_bias(),
        adverse.precommit_cancelled(),
        adverse.accepted_cpu_wait_completed(),
        adverse.pending_handler_completed(),
        adverse.drain_error_no_ack(),
        adverse.handler_error_no_ack(),
        adverse.ack_error(),
    );
}
