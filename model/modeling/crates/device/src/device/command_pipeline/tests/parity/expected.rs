use pdma_model_contract::abi::{
    AbiBounds, AbiCompletion, AbiEventAccess, AbiEventKind, AbiEventStage, AbiFault, AbiFaultSite,
    DmaAddress,
};
use pdma_model_contract::semantic::TraceEventKind;

use super::Observation;
use crate::ModelError;
use crate::device::ModelStepStatus;
use crate::device::command_pipeline::tests::support::{Case, trace_kinds};

pub(super) fn assert_literal(case: Case, observation: &Observation) {
    assert_eq!(observation.destinations, expected_bytes(case));
    match case {
        Case::Copy | Case::Strided | Case::Broadcast | Case::VectorAdd => {
            assert_eq!(observation.terminal, Ok(ModelStepStatus::Progressed));
            assert_eq!(observation.completion, Some(AbiCompletion::success(0, 7)));
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
            let (error, kind, locator) = expected_fault(case);
            assert_eq!(observation.terminal, Err(error));
            assert_eq!(observation.completion, None);
            let event = observation.event.expect("fault event");
            assert_eq!(event.origin_queue_id, 0);
            assert_eq!(event.command_id, 7);
            assert_eq!(event.fault().kind(), kind);
            assert_eq!(event.fault_locator, locator);
            let details = match case {
                Case::UnknownBuffer => (4, 0),
                Case::RangeOverflow => (2, 0),
                Case::OutOfBounds => (8, 0x1_0009),
                Case::Copy | Case::Strided | Case::Broadcast | Case::VectorAdd => {
                    panic!("success case has no fault details")
                }
            };
            assert_eq!((event.detail0, event.detail1), details);
            assert_eq!(
                (event.stage, event.access, event.index),
                (AbiEventStage::Source, AbiEventAccess::Read, 0)
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

fn expected_fault(case: Case) -> (ModelError, AbiEventKind, u64) {
    let site = AbiFaultSite::new(AbiEventStage::Source, AbiEventAccess::Read, 0);
    match case {
        Case::UnknownBuffer => (
            ModelError::MemoryFault(AbiFault::unmapped(DmaAddress::new(0xdead_0000), 4, site)),
            AbiEventKind::UnmappedDmaAddress,
            0xdead_0000,
        ),
        Case::RangeOverflow => (
            ModelError::MemoryFault(AbiFault::range_overflow_direct(
                DmaAddress::new(u64::MAX),
                2,
                site,
            )),
            AbiEventKind::RangeOverflow,
            u64::MAX,
        ),
        Case::OutOfBounds => (
            ModelError::MemoryFault(AbiFault::out_of_bounds(
                DmaAddress::new(0x1_0008),
                AbiBounds::new(8, 0x1_0009),
                site,
            )),
            AbiEventKind::OutOfBounds,
            0x1_0008,
        ),
        Case::Copy | Case::Strided | Case::Broadcast | Case::VectorAdd => panic!("success case"),
    }
}
