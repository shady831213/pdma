use pdma_model_contract::semantic::{
    BufferId, CommandId, CompletionRecord, CompletionStatus, EventErrorKind, EventRecord,
    FaultAddress, MemoryFault, TraceEvent, TraceEventKind,
};
use pdma_model_device::{ModelError, PipelineTerminal};

use super::support::{Case, Drive, Observation, observe, prepare};

#[test]
fn drive_policy_parity() -> Result<(), String> {
    // Given: every accepted semantic operation plus a memory-fault retirement route.
    for case in Case::ALL {
        // When: the same command runs under the legacy and both generic drive policies.
        let legacy = observe(prepare(case), Drive::Legacy)?;
        let stepped = observe(prepare(case), Drive::Stepped)?;
        let immediate = observe(prepare(case), Drive::Immediate)?;

        // Then: terminal state, bytes, records, IRQ state, and trace order are identical.
        assert_eq!(stepped, legacy, "stepped case {case:?}");
        assert_eq!(immediate, legacy, "immediate case {case:?}");
        assert_literal(case, &legacy);
        assert_eq!(
            stepped.trace.first().map(TraceEvent::kind),
            Some(TraceEventKind::CommandSubmitted)
        );
        assert_eq!(
            stepped
                .trace
                .iter()
                .filter(|event| event.kind() == TraceEventKind::Scheduled)
                .count(),
            1
        );
    }
    println!(
        "drive_policy_parity cases=7 copy=1 strided=1 broadcast=1 vector=1 faults=3 bytes=exact records=exact irq=exact trace_order=exact executors=step,immediate legacy=equal"
    );
    Ok(())
}

fn assert_literal(case: Case, observation: &Observation) {
    let queue = observation
        .completion
        .map(CompletionRecord::queue_id)
        .or_else(|| observation.event.map(EventRecord::queue_id));
    let expected_bytes = match case {
        Case::Copy => [vec![1, 2, 3, 4, 0, 0, 0, 0], vec![0; 8]],
        Case::Strided => [vec![1, 2, 0, 4, 5, 0, 0, 0], vec![0; 8]],
        Case::Broadcast => [vec![2, 3, 4, 0, 0, 0, 0, 0], vec![0, 0, 2, 3, 4, 0, 0, 0]],
        Case::VectorAdd => [vec![9, 9, 9, 9, 0, 0, 0, 0], vec![0; 8]],
        Case::UnknownBuffer | Case::RangeOverflow | Case::OutOfBounds => [vec![0; 8], vec![0; 8]],
    };
    assert_eq!(observation.destinations, expected_bytes);
    match case {
        Case::Copy | Case::Strided | Case::Broadcast | Case::VectorAdd => {
            let queue = queue.expect("success queue");
            assert_eq!(observation.terminal, Ok(PipelineTerminal::Progressed));
            assert_eq!(
                observation.completion,
                Some(CompletionRecord::new(
                    queue,
                    CommandId::new(7),
                    CompletionStatus::Success
                ))
            );
            assert_eq!(observation.event, None);
            assert!(observation.irq);
            assert_eq!(
                kinds(&observation.trace),
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
            let (error, fault) = match case {
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
                    return;
                }
            };
            assert_eq!(observation.terminal, Err(error));
            assert_eq!(observation.completion, None);
            assert_eq!(
                observation.event,
                Some(EventRecord::from_memory_fault(
                    queue,
                    CommandId::new(7),
                    fault
                ))
            );
            assert!(!observation.irq);
            assert_eq!(
                kinds(&observation.trace),
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

fn kinds(trace: &[TraceEvent]) -> Vec<TraceEventKind> {
    trace.iter().map(TraceEvent::kind).collect()
}
