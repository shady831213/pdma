use pdma_model_contract::abi::{AbiCompletion, AbiEvent};
use pdma_model_contract::semantic::{TraceEvent, TraceEventKind};

use super::support::{Case, Fixture, PipelineRunOutcome, Prepared, prepare, read};
use crate::device::ModelStepStatus;
use crate::{ModelError, ModelResult};

mod asynchronous;
mod expected;
mod report;

type CursorState = ((usize, usize), (usize, usize));

#[derive(Debug, PartialEq, Eq)]
struct Observation {
    terminal: Result<ModelStepStatus, ModelError>,
    destinations: [Vec<u8>; 2],
    completion: Option<AbiCompletion>,
    event: Option<AbiEvent>,
    cursors_before_poll: CursorState,
    cursors_after_poll: CursorState,
    irq_before_poll: bool,
    irq_after_poll: bool,
    trace: Vec<TraceEvent>,
}

#[derive(Debug)]
struct Driven {
    observation: Observation,
    fetches: usize,
    executions: usize,
    pending: usize,
    post_terminal: PostTerminal,
}

#[derive(Clone, Copy, Debug)]
enum DrivePolicy {
    CoreDrive,
    Stepped,
    Immediate,
    Asynchronous,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PostTerminal {
    LiveCore,
    Exhausted,
}

pub(super) async fn drive_policy_parity() {
    // Given: every supported operation and execution-memory fault class.
    for case in Case::ALL {
        // When: the current core drive and both generic executors drive identical direct cores.
        let core_drive = observe(prepare(case), DrivePolicy::CoreDrive);
        let stepped = observe(prepare(case), DrivePolicy::Stepped);
        let immediate = observe(prepare(case), DrivePolicy::Immediate);
        let asynchronous = asynchronous::observe(prepare(case)).await;

        // Then: literal bytes, records, IRQ state, trace order, and outcomes agree exactly.
        assert_eq!(
            stepped.observation, core_drive.observation,
            "stepped {case:?}"
        );
        assert_eq!(
            immediate.observation, core_drive.observation,
            "immediate {case:?}"
        );
        assert_eq!(
            asynchronous.observation, core_drive.observation,
            "asynchronous {case:?}"
        );
        assert_metrics(&core_drive, PostTerminal::LiveCore);
        assert_metrics(&stepped, PostTerminal::Exhausted);
        assert_metrics(&immediate, PostTerminal::Exhausted);
        assert_metrics(&asynchronous, PostTerminal::Exhausted);
        expected::assert_literal(case, &core_drive.observation);
        report::print(case, DrivePolicy::CoreDrive, &core_drive);
        report::print(case, DrivePolicy::Stepped, &stepped);
        report::print(case, DrivePolicy::Immediate, &immediate);
        report::print(case, DrivePolicy::Asynchronous, &asynchronous);
    }
}

fn observe(mut prepared: Prepared, drive: DrivePolicy) -> Driven {
    let (terminal, pending) = match drive {
        DrivePolicy::CoreDrive => (prepared.fixture.core.step(), 0),
        DrivePolicy::Stepped => drive_stepped(&mut prepared.fixture),
        DrivePolicy::Immediate => match prepared.fixture.immediate() {
            PipelineRunOutcome::Complete(result) => (result, 0),
            other => panic!("unexpected immediate outcome {other:?}"),
        },
        DrivePolicy::Asynchronous => panic!("async policy has its own driver"),
    };
    finish_observation(prepared, terminal, pending, drive)
}

fn finish_observation(
    mut prepared: Prepared,
    terminal: ModelResult<ModelStepStatus>,
    pending: usize,
    drive: DrivePolicy,
) -> Driven {
    let trace = prepared.fixture.core.trace_events();
    let fetches = prepared.fixture.fetch_count();
    let executions = trace
        .iter()
        .filter(|event| {
            matches!(
                event.kind(),
                TraceEventKind::ExecutionSucceeded | TraceEventKind::ExecutionFaulted
            )
        })
        .count();
    let cursors_before_poll = crate::device::command_pipeline::tests::support::cursors(
        &prepared.fixture.core,
        prepared.queue,
    );
    let irq_before_poll = prepared.fixture.core.irq_snapshot().is_some();
    let destinations = [
        read(&prepared.fixture.core, prepared.destinations[0]).expect("destination A"),
        read(&prepared.fixture.core, prepared.destinations[1]).expect("destination B"),
    ];
    let completion =
        crate::test_support::poll_completion(&mut prepared.fixture.core, prepared.queue);
    let event = crate::test_support::poll_event(&mut prepared.fixture.core, prepared.queue);
    let cursors_after_poll = crate::device::command_pipeline::tests::support::cursors(
        &prepared.fixture.core,
        prepared.queue,
    );
    let irq_after_poll = prepared.fixture.core.irq_snapshot().is_some();
    let post_terminal = match drive {
        DrivePolicy::CoreDrive => PostTerminal::LiveCore,
        DrivePolicy::Stepped | DrivePolicy::Asynchronous => {
            assert_eq!(prepared.fixture.step(), PipelineRunOutcome::Exhausted);
            PostTerminal::Exhausted
        }
        DrivePolicy::Immediate => {
            assert_eq!(prepared.fixture.immediate(), PipelineRunOutcome::Exhausted);
            PostTerminal::Exhausted
        }
    };
    Driven {
        observation: Observation {
            terminal,
            destinations,
            completion,
            event,
            cursors_before_poll,
            cursors_after_poll,
            irq_before_poll,
            irq_after_poll,
            trace,
        },
        fetches,
        executions,
        pending,
        post_terminal,
    }
}

fn drive_stepped(fixture: &mut Fixture) -> (ModelResult<ModelStepStatus>, usize) {
    let mut pending = 0;
    loop {
        match fixture.step() {
            PipelineRunOutcome::Progressed => {}
            PipelineRunOutcome::Pending(_) => pending += 1,
            PipelineRunOutcome::Complete(result) => return (result, pending),
            other => panic!("unexpected stepped outcome {other:?}"),
        }
    }
}

fn assert_metrics(driven: &Driven, post_terminal: PostTerminal) {
    assert_eq!(driven.fetches, 1);
    assert_eq!(driven.executions, 1);
    assert_eq!(driven.pending, 0);
    assert_eq!(driven.post_terminal, post_terminal);
}
