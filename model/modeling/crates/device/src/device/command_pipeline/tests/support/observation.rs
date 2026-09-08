use pdma_model_contract::semantic::TraceEvent;

use super::{Fixture, PipelineRunOutcome};
use crate::device::{DeviceCore, ModelStepStatus};
use crate::{MappedDmaRegion, ModelError, ModelResult};

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

pub(crate) fn read(core: &DeviceCore, region: MappedDmaRegion) -> ModelResult<Vec<u8>> {
    core.memory.read_object(region.object(), 0, 8)
}
