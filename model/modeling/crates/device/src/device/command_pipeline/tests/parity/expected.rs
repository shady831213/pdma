use pdma_model_contract::semantic::{
    BufferId, CommandId, CompletionRecord, CompletionStatus, EventErrorKind, EventRecord,
    FaultAddress, MemoryFault, TraceEventKind,
};

use super::Observation;
use crate::ModelError;
use crate::device::ModelStepStatus;
use crate::device::command_pipeline::tests::support::{Case, trace_kinds};

pub(super) fn assert_literal(case: Case, observation: &Observation) {
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
