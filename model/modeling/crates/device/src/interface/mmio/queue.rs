use crate::device::DeviceCore;
use crate::register_file::QueueStaging;
use pdma_model_contract::abi::mmio::QueueResponseBank;

pub(crate) mod batch;
mod queue_config;
pub(crate) mod queue_control;
pub(crate) use batch::{BootstrapBatchStaging, SubmissionBatchStaging};
pub(crate) use queue_control::map_bus as map_bus_error;

pub(crate) fn status_response(
    status: crate::queue::exact::QueueStatus,
) -> Result<QueueResponseBank, pdma_model_contract::abi::mmio::MmioResultDetails> {
    Ok(QueueResponseBank {
        epoch: status.queue().epoch().raw().into(),
        queue_id: u64::try_from(status.queue().queue_id().raw())
            .map_err(|_| {
                queue_control::fail(
                    pdma_model_contract::abi::mmio::MmioResult::InvalidTransaction,
                    [0; 3],
                )
            })?
            .into(),
        sq_producer: status.submission().producer().raw().into(),
        sq_captured: status.captured_submission_producer().raw().into(),
        sq_consumer: status.submission().consumer().raw().into(),
        cq_producer: status.completion().producer().raw().into(),
        cq_consumer: status.completion().consumer().raw().into(),
        eq_producer: status.event().producer().raw().into(),
        eq_consumer: status.event().consumer().raw().into(),
    })
}

pub(super) fn execute(state: &mut DeviceCore, value: u64) -> bool {
    let staging = state.registers.queue.staging();
    let (result, response, changed) = queue_control::execute(state, staging, value);
    state.registers.queue.command.complete(result, response);
    changed
}
