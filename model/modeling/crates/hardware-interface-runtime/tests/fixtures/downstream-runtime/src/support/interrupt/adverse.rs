use super::super::error::{ConsumerError, CpuCounts, lock};
use super::{Handler, Lifecycle, Notice, Source, SourceState};
use hir::{
    HandlerRegistry, InterruptDispatchError, InterruptDispatcher, InterruptJoinError, SoftwareCpu,
    TraceBuffer,
};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicUsize, Ordering},
};
use tokio::sync::oneshot;

pub(in crate::support::interrupt) async fn run_adverse_dispatch() -> Result<(), ConsumerError> {
    let source_state = Arc::new(Mutex::new(SourceState::default()));
    let source_drops = Arc::new(AtomicUsize::new(0));
    let handler_entries = Arc::new(AtomicUsize::new(0));
    let handler_drops = Arc::new(AtomicUsize::new(0));
    let lifecycle_events = Arc::new(AtomicUsize::new(0));
    let cpu = Arc::new(SoftwareCpu::new());
    let gate_local = cpu.local(CpuCounts::default());
    let (gate_held_tx, gate_held_rx) = oneshot::channel();
    let (gate_release_tx, gate_release_rx) = oneshot::channel();
    let blocker = hold_cpu_gate(
        Arc::clone(&cpu),
        gate_local.clone(),
        gate_held_tx,
        gate_release_rx,
    );
    gate_held_rx
        .await
        .map_err(|_| ConsumerError::Invariant("adverse CPU gate was not held"))?;
    let (claimed_tx, claimed_rx) = oneshot::channel();
    lock(&source_state).claimed = Some(claimed_tx);
    let mut registry = HandlerRegistry::<u8, Notice, Handler>::new();
    registry
        .register(
            1,
            Handler::immediate(Arc::clone(&handler_entries), Arc::clone(&handler_drops)),
        )
        .map_err(|_| ConsumerError::Invariant("adverse registry duplicate"))?;
    let adverse_trace = TraceBuffer::new();
    let task = InterruptDispatcher::start(
        Arc::clone(&cpu),
        registry
            .freeze()
            .map_err(|_| ConsumerError::Invariant("adverse registry empty"))?,
        Source::one(9, Arc::clone(&source_state), Arc::clone(&source_drops)),
        Lifecycle {
            events: Arc::clone(&lifecycle_events),
            trace: adverse_trace.clone(),
        }
        .observer(),
    );
    claimed_rx
        .await
        .map_err(|_| ConsumerError::Invariant("adverse source was not claimed"))?;
    let outcome = task.shutdown_and_join().await;
    let dispatcher_bypassed_held_gate = matches!(
        outcome,
        Err(InterruptJoinError::Dispatch(
            InterruptDispatchError::UnregisteredVector(9)
        ))
    );
    let dispatcher_cpu_entries = usize::from(u8::from(!dispatcher_bypassed_held_gate));
    let _ = gate_release_tx.send(());
    blocker
        .await
        .map_err(|_| ConsumerError::Invariant("adverse CPU gate task did not join"))??;
    let gate_state = cpu
        .run_handler_turn(async { gate_local.with(|state| *state) })
        .await?;
    ensure(
        dispatcher_bypassed_held_gate,
        "unregistered interrupt did not resolve before the held CPU gate released",
    )?;
    ensure(
        lock(&source_state).consumed == 1,
        "unregistered source was not claimed",
    )?;
    ensure(
        handler_entries.load(Ordering::SeqCst) == 0 && dispatcher_cpu_entries == 0,
        "unregistered interrupt entered the handler or CPU gate",
    )?;
    ensure(
        lifecycle_events.load(Ordering::SeqCst) == 0,
        "unregistered lifecycle entered",
    )?;
    ensure(
        adverse_trace.events().is_empty(),
        "unregistered interrupt recorded an adverse trace event",
    )?;
    ensure(
        gate_state
            == CpuCounts {
                main_segments: 1,
                handler_entries: 0,
            },
        "adverse CPU gate was not held by a real CPU-local turn",
    )?;
    ensure(
        source_drops.load(Ordering::SeqCst) == 1 && handler_drops.load(Ordering::SeqCst) == 1,
        "adverse resources were not released",
    )
}

fn hold_cpu_gate(
    cpu: Arc<SoftwareCpu>,
    local: hir::CpuLocal<CpuCounts>,
    held: oneshot::Sender<()>,
    release: oneshot::Receiver<()>,
) -> tokio::task::JoinHandle<Result<(), ConsumerError>> {
    tokio::spawn(async move {
        cpu.run_handler_turn(async move {
            local.with(|state| state.main_segments = state.main_segments.saturating_add(1))?;
            let _ = held.send(());
            let _ = release.await;
            Ok(())
        })
        .await
    })
}

const fn ensure(condition: bool, message: &'static str) -> Result<(), ConsumerError> {
    if condition {
        Ok(())
    } else {
        Err(ConsumerError::Invariant(message))
    }
}
