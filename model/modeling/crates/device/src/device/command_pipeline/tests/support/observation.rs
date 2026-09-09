use pdma_model_contract::semantic::TraceEvent;

use super::{Fixture, PipelineRunOutcome};
use crate::ModelError;
use crate::device::{DeviceCore, ModelStepStatus};
use pdma_model_contract::abi::DmaRegion;

pub(crate) fn cursors(
    core: &DeviceCore,
    queue: crate::test_support::TestQueue,
) -> ((usize, usize), (usize, usize)) {
    use pdma_model_contract::abi::registers::QueueRegisters;
    let values = [
        QueueRegisters::SQ_CONSUMER.offset(),
        QueueRegisters::SQ_PRODUCER.offset(),
        QueueRegisters::CQ_CONSUMER.offset(),
        QueueRegisters::CQ_PRODUCER.offset(),
    ]
    .map(|offset| {
        usize::try_from(crate::test_support::read_queue(core, queue, offset)).expect("cursor")
    });
    ((values[0], values[1]), (values[2], values[3]))
}

pub(crate) fn drive_to_complete(fixture: &mut Fixture) -> Result<ModelStepStatus, ModelError> {
    loop {
        match fixture.step() {
            PipelineRunOutcome::Progressed => {}
            PipelineRunOutcome::Complete(result) => return result,
            other => panic!("unexpected step result: {other:?}"),
        }
    }
}

pub(crate) fn drive_to_pending(fixture: &mut Fixture) -> PipelineRunOutcome {
    loop {
        match fixture.step() {
            PipelineRunOutcome::Progressed => {}
            outcome => return outcome,
        }
    }
}

pub(crate) fn trace_kinds(
    trace: &[TraceEvent],
) -> Vec<pdma_model_contract::semantic::TraceEventKind> {
    trace.iter().map(TraceEvent::kind).collect()
}

pub(crate) fn read(
    core: &DeviceCore,
    region: DmaRegion,
) -> Result<Vec<u8>, hardware_interface_runtime::MemoryAccessError> {
    crate::memory::read(&core.memory, region.base(), 8)
}
