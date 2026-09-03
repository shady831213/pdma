use pdma_model_contract::semantic::{
    BufferId, CommandId, CompletionRecord, CompletionStatus, EventErrorKind, EventRecord,
    FaultAddress, MemoryFault, TraceEvent, TraceEventKind,
};

use super::support::{Case, Fixture, PipelineRunOutcome, Prepared, prepare, read, trace_kinds};
use crate::device::ModelStepStatus;
use crate::{ModelError, ModelResult};

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
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PostTerminal {
    LiveCore,
    Exhausted,
}

pub(super) fn drive_policy_parity() {
    // Given: every supported operation and execution-memory fault class.
    for case in Case::ALL {
        // When: the legacy core and both generic executors drive identical direct cores.
        let legacy = observe(prepare(case), Drive::Legacy);
        let stepped = observe(prepare(case), Drive::Stepped);
        let immediate = observe(prepare(case), Drive::Immediate);

        // Then: literal bytes, records, IRQ state, trace order, and outcomes agree exactly.
        assert_eq!(stepped.observation, legacy.observation, "stepped {case:?}");
        assert_eq!(
            immediate.observation, legacy.observation,
            "immediate {case:?}"
        );
        assert_metrics(&legacy, PostTerminal::LiveCore);
        assert_metrics(&stepped, PostTerminal::Exhausted);
        assert_metrics(&immediate, PostTerminal::Exhausted);
        assert_literal(case, &legacy.observation);
        report::print(case, Drive::Legacy, &legacy);
        report::print(case, Drive::Stepped, &stepped);
        report::print(case, Drive::Immediate, &immediate);
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
    };
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
        Drive::Stepped => {
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

fn assert_literal(case: Case, observation: &Observation) {
    let queue = observation
        .completion
        .map(CompletionRecord::queue_id)
        .or_else(|| observation.event.map(EventRecord::queue_id));
    assert_eq!(observation.destinations, expected_bytes(case));
    match case {
        Case::Copy | Case::Strided | Case::Broadcast | Case::VectorAdd => {
            let queue = queue.expect("success queue");
            assert_eq!(observation.terminal, Ok(ModelStepStatus::Progressed));
            assert_eq!(
                observation.completion,
                Some(CompletionRecord::new(
                    queue,
                    CommandId::new(7),
                    CompletionStatus::Success,
                ))
            );
            assert_eq!(observation.event, None);
            assert!(observation.irq_before_poll);
            assert!(observation.irq_after_poll);
            assert_eq!(
                trace_kinds(&observation.trace),
                [
                    TraceEventKind::CommandSubmitted,
                    TraceEventKind::DoorbellRunnable,
                    TraceEventKind::Scheduled,
                    TraceEventKind::ExecutionSucceeded,
                    TraceEventKind::CompletionPublished,
                    TraceEventKind::IrqSourceLatched,
                    TraceEventKind::IrqLineAsserted,
                ]
            );
        }
        Case::UnknownBuffer | Case::RangeOverflow | Case::OutOfBounds => {
            let queue = queue.expect("fault queue");
            let (error, fault) = expected_fault(case);
            assert_eq!(observation.terminal, Err(error));
            assert_eq!(observation.completion, None);
            assert_eq!(
                observation.event,
                Some(EventRecord::from_memory_fault(
                    queue,
                    CommandId::new(7),
                    fault,
                ))
            );
            assert!(!observation.irq_before_poll);
            assert!(!observation.irq_after_poll);
            assert_eq!(
                trace_kinds(&observation.trace),
                [
                    TraceEventKind::CommandSubmitted,
                    TraceEventKind::DoorbellRunnable,
                    TraceEventKind::Scheduled,
                    TraceEventKind::ExecutionFaulted,
                    TraceEventKind::EventPublished,
                ]
            );
        }
    }
}

fn expected_bytes(case: Case) -> [Vec<u8>; 2] {
    match case {
        Case::Copy => [vec![1, 2, 3, 4, 0, 0, 0, 0], vec![0; 8]],
        Case::Strided => [vec![1, 2, 0, 4, 5, 0, 0, 0], vec![0; 8]],
        Case::Broadcast => [vec![2, 3, 4, 0, 0, 0, 0, 0], vec![0, 0, 2, 3, 4, 0, 0, 0]],
        Case::VectorAdd => [vec![9, 9, 9, 9, 0, 0, 0, 0], vec![0; 8]],
        Case::UnknownBuffer | Case::RangeOverflow | Case::OutOfBounds => [vec![0; 8], vec![0; 8]],
    }
}

fn expected_fault(case: Case) -> (ModelError, MemoryFault) {
    match case {
        Case::UnknownBuffer => (
            ModelError::UnknownBuffer(BufferId::new(999)),
            MemoryFault::new(BufferId::new(999), EventErrorKind::UnknownBuffer),
        ),
        Case::RangeOverflow => (
            ModelError::RangeOverflow,
            MemoryFault::new(BufferId::new(0), EventErrorKind::RangeOverflow),
        ),
        Case::OutOfBounds => (
            ModelError::OutOfBounds {
                buffer: BufferId::new(0),
                len: 8,
                end: 9,
            },
            MemoryFault::with_address(
                BufferId::new(0),
                EventErrorKind::OutOfBounds { len: 8, end: 9 },
                FaultAddress::new(0x1_0008),
            ),
        ),
        Case::Copy | Case::Strided | Case::Broadcast | Case::VectorAdd => {
            panic!("success case has no fault")
        }
    }
}
