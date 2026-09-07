use pdma_model_contract::semantic::{CompletionRecord, EventRecord, TraceEvent, TraceEventKind};

use super::support::{Case, Fixture, PipelineRunOutcome, Prepared, prepare, read};
use crate::device::ModelStepStatus;
use crate::{ModelError, ModelResult};

mod asynchronous;
mod expected;
mod report;

type CursorState = Result<((usize, usize), (usize, usize)), ModelError>;

#[derive(Debug, PartialEq, Eq)]
struct Observation {
    terminal: Result<ModelStepStatus, ModelError>,
    destinations: [Vec<u8>; 2],
    completion: Option<CompletionRecord>,
    event: Option<EventRecord>,
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
enum Drive {
    Legacy,
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
        // When: the legacy core and both generic executors drive identical direct cores.
        let legacy = observe(prepare(case), Drive::Legacy);
        let stepped = observe(prepare(case), Drive::Stepped);
        let immediate = observe(prepare(case), Drive::Immediate);
        let asynchronous = asynchronous::observe(prepare(case)).await;

        // Then: literal bytes, records, IRQ state, trace order, and outcomes agree exactly.
        assert_eq!(stepped.observation, legacy.observation, "stepped {case:?}");
        assert_eq!(
            immediate.observation, legacy.observation,
            "immediate {case:?}"
        );
        assert_eq!(
            asynchronous.observation, legacy.observation,
            "asynchronous {case:?}"
        );
        assert_metrics(&legacy, PostTerminal::LiveCore);
        assert_metrics(&stepped, PostTerminal::Exhausted);
        assert_metrics(&immediate, PostTerminal::Exhausted);
        assert_metrics(&asynchronous, PostTerminal::Exhausted);
        expected::assert_literal(case, &legacy.observation);
        report::print(case, Drive::Legacy, &legacy);
        report::print(case, Drive::Stepped, &stepped);
        report::print(case, Drive::Immediate, &immediate);
        report::print(case, Drive::Asynchronous, &asynchronous);
    }
}

fn observe(mut prepared: Prepared, drive: Drive) -> Driven {
    let (terminal, pending) = match drive {
        Drive::Legacy => (prepared.fixture.core.step(), 0),
        Drive::Stepped => drive_stepped(&mut prepared.fixture),
        Drive::Immediate => match prepared.fixture.immediate() {
            PipelineRunOutcome::Complete(result) => (result, 0),
            other => panic!("unexpected immediate outcome {other:?}"),
        },
        Drive::Asynchronous => panic!("async policy has its own driver"),
    };
    finish_observation(prepared, terminal, pending, drive)
}

fn finish_observation(
    mut prepared: Prepared,
    terminal: ModelResult<ModelStepStatus>,
    pending: usize,
    drive: Drive,
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
    let cursors_before_poll = prepared
        .fixture
        .core
        .queues
        .semantic_cursors(prepared.queue.queue_id());
    let irq_before_poll = prepared.fixture.core.irq_snapshot().is_some();
    let destinations = [
        read(&prepared.fixture.core, prepared.destinations[0]).expect("destination A"),
        read(&prepared.fixture.core, prepared.destinations[1]).expect("destination B"),
    ];
    let completion = prepared
        .fixture
        .core
        .poll_completion(prepared.queue)
        .expect("completion");
    let event = prepared
        .fixture
        .core
        .poll_queue_event(prepared.queue)
        .expect("event");
    let cursors_after_poll = prepared
        .fixture
        .core
        .queues
        .semantic_cursors(prepared.queue.queue_id());
    let irq_after_poll = prepared.fixture.core.irq_snapshot().is_some();
    let post_terminal = match drive {
        Drive::Legacy => PostTerminal::LiveCore,
        Drive::Stepped | Drive::Asynchronous => {
            assert_eq!(prepared.fixture.step(), PipelineRunOutcome::Exhausted);
            PostTerminal::Exhausted
        }
        Drive::Immediate => {
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
