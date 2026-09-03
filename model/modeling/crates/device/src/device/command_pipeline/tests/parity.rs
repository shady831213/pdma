use pdma_model_contract::semantic::{
    BufferId, CommandId, CompletionRecord, CompletionStatus, EventErrorKind, EventRecord,
    FaultAddress, MemoryFault, TraceEvent, TraceEventKind,
};

use super::support::{Case, Fixture, PipelineRunOutcome, Prepared, prepare, read, trace_kinds};
use crate::device::ModelStepStatus;
use crate::{ModelError, ModelResult};

#[derive(Debug, PartialEq, Eq)]
struct Observation {
    terminal: Result<ModelStepStatus, ModelError>,
    destinations: [Vec<u8>; 2],
    completion: Option<CompletionRecord>,
    event: Option<EventRecord>,
    irq: bool,
    trace: Vec<TraceEvent>,
}

#[derive(Clone, Copy)]
enum Drive {
    Legacy,
    Stepped,
    Immediate,
}

pub(super) fn drive_policy_parity() {
    // Given: every supported operation and execution-memory fault class.
    for case in Case::ALL {
        // When: the legacy core and both generic executors drive identical direct cores.
        let legacy = observe(prepare(case), Drive::Legacy);
        let stepped = observe(prepare(case), Drive::Stepped);
        let immediate = observe(prepare(case), Drive::Immediate);

        // Then: literal bytes, records, IRQ state, trace order, and outcomes agree exactly.
        assert_eq!(stepped, legacy, "stepped {case:?}");
        assert_eq!(immediate, legacy, "immediate {case:?}");
        assert_literal(case, &legacy);
    }
    println!(
        "drive_policy_parity cases=7 copy=1 strided=1 broadcast=1 vector=1 faults=3 bytes=literal records=literal irq=literal trace_order=literal executors=step,immediate legacy=equal"
    );
}

fn observe(mut prepared: Prepared, drive: Drive) -> Observation {
    let terminal = match drive {
        Drive::Legacy => prepared.fixture.core.step(),
        Drive::Stepped => drive_stepped(&mut prepared.fixture),
        Drive::Immediate => match prepared.fixture.immediate() {
            PipelineRunOutcome::Complete(result) => result,
            other => panic!("unexpected immediate outcome {other:?}"),
        },
    };
    let trace = prepared.fixture.core.trace_events();
    Observation {
        terminal,
        destinations: [
            read(&prepared.fixture.core, prepared.destinations[0]).expect("destination A"),
            read(&prepared.fixture.core, prepared.destinations[1]).expect("destination B"),
        ],
        completion: prepared
            .fixture
            .core
            .poll_completion(prepared.queue)
            .expect("completion"),
        event: prepared
            .fixture
            .core
            .poll_queue_event(prepared.queue)
            .expect("event"),
        irq: prepared.fixture.core.irq_snapshot().is_some(),
        trace,
    }
}

fn drive_stepped(fixture: &mut Fixture) -> ModelResult<ModelStepStatus> {
    loop {
        match fixture.step() {
            PipelineRunOutcome::Progressed => {}
            PipelineRunOutcome::Complete(result) => return result,
            other => panic!("unexpected stepped outcome {other:?}"),
        }
    }
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
            assert!(observation.irq);
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
            assert!(!observation.irq);
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
