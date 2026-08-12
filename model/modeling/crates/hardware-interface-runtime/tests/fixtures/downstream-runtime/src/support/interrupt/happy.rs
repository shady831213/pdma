use super::super::error::{ConsumerError, CpuCounts, lock};
use super::{Handler, Lifecycle, Notice, Source, SourceState};
use hir::{HandlerRegistry, InterruptDispatcher, SoftwareCpu, TraceBuffer};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicUsize, Ordering},
};
use tokio::sync::oneshot;

pub(in crate::support::interrupt) async fn run_happy_dispatch(
    trace: &TraceBuffer<(usize, &'static str)>,
) -> Result<usize, ConsumerError> {
    let cpu = Arc::new(SoftwareCpu::new());
    let counts = cpu.local(CpuCounts::default());
    let (before_tx, before_rx) = oneshot::channel();
    let (resume_tx, resume_rx) = oneshot::channel();
    let (after_tx, mut after_rx) = oneshot::channel();
    let main = spawn_main(
        Arc::clone(&cpu),
        counts.clone(),
        trace.clone(),
        before_tx,
        resume_rx,
        after_tx,
    );
    before_rx
        .await
        .map_err(|_| ConsumerError::Invariant("main did not begin"))?;
    let (handler_started_tx, handler_started_rx) = oneshot::channel();
    let (handler_release_tx, handler_release_rx) = oneshot::channel();
    let handler_entries = Arc::new(AtomicUsize::new(0));
    let handler_drops = Arc::new(AtomicUsize::new(0));
    let source_state = Arc::new(Mutex::new(SourceState::default()));
    let source_drops = Arc::new(AtomicUsize::new(0));
    let mut registry = HandlerRegistry::<u8, Notice, Handler>::new();
    registry
        .register(
            1,
            Handler::pending(
                Arc::clone(&handler_entries),
                counts,
                handler_release_rx,
                handler_started_tx,
                Arc::clone(&handler_drops),
            ),
        )
        .map_err(|_| ConsumerError::Invariant("happy registry duplicate"))?;
    let lifecycle_events = Arc::new(AtomicUsize::new(0));
    let dispatcher = InterruptDispatcher::start(
        Arc::clone(&cpu),
        registry
            .freeze()
            .map_err(|_| ConsumerError::Invariant("happy registry empty"))?,
        Source::one(1, Arc::clone(&source_state), Arc::clone(&source_drops)),
        Lifecycle {
            events: Arc::clone(&lifecycle_events),
            trace: trace.clone(),
        }
        .observer(),
    );
    handler_started_rx
        .await
        .map_err(|_| ConsumerError::Invariant("handler did not begin"))?;
    let _ = resume_tx.send(());
    ensure(
        after_rx.try_recv().is_err(),
        "main interleaved with pending handler",
    )?;
    let _ = handler_release_tx.send(());
    let summary = dispatcher
        .shutdown_and_join()
        .await
        .map_err(|_| ConsumerError::Invariant("happy dispatcher failed"))?;
    let snapshot = after_rx
        .await
        .map_err(|_| ConsumerError::Invariant("main did not resume"))?;
    main.await
        .map_err(|_| ConsumerError::Invariant("main task did not join"))??;
    ensure(summary.deliveries() == 1, "happy delivery count changed")?;
    ensure(
        snapshot
            == CpuCounts {
                main_segments: 2,
                handler_entries: 1,
            },
        "CPU-local state changed",
    )?;
    ensure(
        lock(&source_state).consumed == 1,
        "source claim count changed",
    )?;
    ensure(
        handler_entries.load(Ordering::SeqCst) == 1,
        "handler CPU entry changed",
    )?;
    ensure(
        lifecycle_events.load(Ordering::SeqCst) == 2,
        "handler lifecycle changed",
    )?;
    ensure(
        source_drops.load(Ordering::SeqCst) == 1 && handler_drops.load(Ordering::SeqCst) == 1,
        "dispatcher resources were not released",
    )?;
    Ok(summary.deliveries())
}

fn spawn_main(
    cpu: Arc<SoftwareCpu>,
    counts: hir::CpuLocal<CpuCounts>,
    trace: TraceBuffer<(usize, &'static str)>,
    before: oneshot::Sender<()>,
    resume: oneshot::Receiver<()>,
    after: oneshot::Sender<CpuCounts>,
) -> tokio::task::JoinHandle<Result<(), ConsumerError>> {
    tokio::spawn(async move {
        cpu.run_main(async move {
            counts.with(|state| state.main_segments = state.main_segments.saturating_add(1))?;
            trace.push_with(|sequence| (sequence, "main-before"));
            let _ = before.send(());
            let _ = resume.await;
            let snapshot = counts.with(|state| {
                state.main_segments = state.main_segments.saturating_add(1);
                *state
            })?;
            trace.push_with(|sequence| (sequence, "main-after"));
            let _ = after.send(snapshot);
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
