use super::super::error::{ConsumerError, EnvCounts, lock};
use super::{Handler, Lifecycle, Notice, Source, SourceState};
use hir::{
    HandlerRegistry, InterruptDispatchError, InterruptDispatcher, InterruptJoinError, SoftwareEnv,
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
    let env = Arc::new(SoftwareEnv::new());
    let gate_local = env.local(EnvCounts::default());
    let (gate_held_tx, gate_held_rx) = oneshot::channel();
    let (gate_release_tx, gate_release_rx) = oneshot::channel();
    let blocker = hold_env_gate(
        Arc::clone(&env),
        gate_local.clone(),
        gate_held_tx,
        gate_release_rx,
    );
    gate_held_rx
        .await
        .map_err(|_| ConsumerError::Invariant("adverse environment gate was not held"))?;
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
        Arc::clone(&env),
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
    let dispatcher_env_entries = usize::from(u8::from(!dispatcher_bypassed_held_gate));
    let _ = gate_release_tx.send(());
    blocker
        .await
        .map_err(|_| ConsumerError::Invariant("adverse environment gate task did not join"))??;
    let gate_state = env
        .run_handler_turn(async { gate_local.with(|state| *state) })
        .await?;
    ensure(
        dispatcher_bypassed_held_gate,
        "unregistered interrupt did not resolve before the held environment gate released",
    )?;
    ensure(
        lock(&source_state).consumed == 1,
        "unregistered source was not claimed",
    )?;
    ensure(
        handler_entries.load(Ordering::SeqCst) == 0 && dispatcher_env_entries == 0,
        "unregistered interrupt entered the handler or environment gate",
    )?;
    ensure(
        lifecycle_events.load(Ordering::SeqCst) == 0,
        "unregistered lifecycle entered",
    )?;
    ensure(
        adverse_trace.snapshot().is_empty(),
        "unregistered interrupt recorded an adverse trace event",
    )?;
    ensure(
        gate_state
            == EnvCounts {
                main_segments: 1,
                handler_entries: 0,
            },
        "adverse environment gate was not held by a real environment-local turn",
    )?;
    ensure(
        source_drops.load(Ordering::SeqCst) == 1 && handler_drops.load(Ordering::SeqCst) == 1,
        "adverse resources were not released",
    )
}

fn hold_env_gate(
    env: Arc<SoftwareEnv>,
    local: hir::EnvLocal<EnvCounts>,
    held: oneshot::Sender<()>,
    release: oneshot::Receiver<()>,
) -> tokio::task::JoinHandle<Result<(), ConsumerError>> {
    tokio::spawn(async move {
        env.run_handler_turn(async move {
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
