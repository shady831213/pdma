use super::*;
use hardware_interface_runtime::{NoopRegisterObserver, ObservationGate, SharedRegisterObserver};
use pdma_model_contract::semantic::{
    Command, CommandId, CommandOperation, DeviceEpoch, QueueConfig, ResourceConfig,
    SoftwareOwnerId, TraceEventKind, WholeDeviceReset, desc::CopyRequest, queue::EventRouting,
};

struct AcceptedCopy {
    shared: RunnerShared,
    target: pdma_model_contract::semantic::BufferId,
}

fn accepted_copy() -> AcceptedCopy {
    let mut core = crate::DeviceCore::new(
        std::sync::Arc::new(hardware_interface_runtime::SharedMemory::new()),
        ResourceConfig::default(),
    );
    core.enable_tracing();
    let source = core.dma.allocate(4).expect("source allocation");
    let target = core.dma.allocate(4).expect("target allocation");
    core.dma
        .write_object(source.object(), 0, &[1, 3, 5, 7])
        .expect("source bytes");
    let queue = core
        .create_queue(QueueConfig::new(
            2,
            2,
            EventRouting::QueueLocal,
            SoftwareOwnerId::new(0),
        ))
        .expect("queue");
    core.submit_command(
        queue,
        Command::new(
            CommandId::new(9),
            CommandOperation::Copy(CopyRequest::new(source.object(), target.object(), 4)),
        ),
    )
    .expect("submission");
    core.ring_doorbell(queue).expect("doorbell");
    AcceptedCopy {
        shared: RunnerShared {
            backend: crate::runtime::backend::DeviceInteractionBackend::new(RunnerState::new(core)),
            register_gate: ObservationGate::new(SharedRegisterObserver::new(std::sync::Arc::new(
                NoopRegisterObserver,
            ))),
        },
        target: target.object(),
    }
}

async fn observation(fixture: &AcceptedCopy) -> (usize, Vec<u8>, usize, usize) {
    fixture
        .shared
        .backend
        .interact(|state| {
            (
                state.core.native_steps(),
                state
                    .core
                    .dma
                    .read_object(fixture.target, 0, 4)
                    .expect("target bytes"),
                state.steps,
                state
                    .core
                    .trace_events()
                    .iter()
                    .filter(|event| event.kind() == TraceEventKind::CompletionPublished)
                    .count(),
            )
        })
        .await
}

#[tokio::test(flavor = "current_thread")]
async fn accepted_command_advances_one_leaf_per_worker_turn() {
    // Given: one accepted copy whose observable stages are fetch, execute, and retire.
    let fixture = accepted_copy();

    // When: the worker is advanced through three deterministic interaction turns.
    let mut observations = Vec::new();
    for _ in 0..3 {
        assert!(run_one(&fixture.shared).await);
        observations.push(observation(&fixture).await);
    }

    // Then: each completed stage is retained and only terminal progress is public.
    assert_eq!(
        observations,
        [
            (1, vec![0, 0, 0, 0], 0, 0),
            (1, vec![1, 3, 5, 7], 0, 0),
            (1, vec![1, 3, 5, 7], 1, 1),
        ]
    );
}

#[tokio::test(flavor = "current_thread")]
async fn reset_and_shutdown_discard_retained_stages_without_late_effects() {
    // Given: copies retained immediately after fetch and immediately after execution.
    let before_execution = accepted_copy();
    assert!(run_one(&before_execution.shared).await);
    let after_execution = accepted_copy();
    for _ in 0..2 {
        assert!(run_one(&after_execution.shared).await);
    }
    assert_eq!(observation(&before_execution).await, (1, vec![0; 4], 0, 0));
    assert_eq!(
        observation(&after_execution).await,
        (1, vec![1, 3, 5, 7], 0, 0)
    );

    // When: reset clears both retained stages and shutdown clears a third fetched stage.
    for fixture in [&before_execution, &after_execution] {
        fixture
            .shared
            .backend
            .interact(|state| {
                state
                    .core
                    .reset_runner(WholeDeviceReset::new(DeviceEpoch::initial()))
                    .expect("reset");
                state.shutdown_requested = true;
            })
            .await;
        assert!(!run_one(&fixture.shared).await);
    }
    let shutdown = accepted_copy();
    assert!(run_one(&shutdown.shared).await);
    shutdown
        .shared
        .backend
        .interact(|state| state.shutdown_requested = true)
        .await;
    assert!(!run_one(&shutdown.shared).await);

    // Then: only the DMA committed before reset persists; no retirement becomes visible.
    assert_eq!(observation(&before_execution).await, (1, vec![0; 4], 0, 0));
    assert_eq!(
        observation(&after_execution).await,
        (1, vec![1, 3, 5, 7], 0, 0)
    );
    assert_eq!(observation(&shutdown).await, (1, vec![0; 4], 0, 0));
    println!("reset_boundaries fetch=1 execute=1 retire=0 shutdown_fetch=1 late_publications=0");
}
