use crate::device::DeviceCore;
use crate::queue::QueueBlock;
use pdma_model_contract::abi::mmio::{
    MmioResultDetails, QueueResponseBank, ResponseUpdate, apply_response_update,
};

mod queue_config;
pub(crate) mod queue_control;

#[derive(Clone, Copy)]
pub(crate) struct QueueRequest {
    pub(crate) expected_epoch: u64,
    pub(crate) id: u64,
    pub(crate) expected_cursor: u64,
    pub(crate) new_cursor: u64,
    pub(crate) cfg_sq_depth: u32,
    pub(crate) cfg_cq_depth: u32,
    pub(crate) cfg_software_owner: u64,
    pub(crate) sq_base: u64,
    pub(crate) sq_depth: u32,
    pub(crate) cq_base: u64,
    pub(crate) cq_depth: u32,
    pub(crate) eq_base: u64,
    pub(crate) eq_depth: u32,
    pub(crate) command: u32,
}

impl QueueBlock {
    pub(crate) const fn snapshot_request(&self) -> QueueRequest {
        let registers = &self.registers;
        QueueRequest {
            expected_epoch: registers.expected_epoch.raw(),
            id: registers.id.raw(),
            expected_cursor: registers.expected_cursor.raw(),
            new_cursor: registers.new_cursor.raw(),
            cfg_sq_depth: registers.cfg_sq_depth.raw(),
            cfg_cq_depth: registers.cfg_cq_depth.raw(),
            cfg_software_owner: registers.cfg_software_owner.raw(),
            sq_base: registers.sq_base.raw(),
            sq_depth: registers.sq_depth.raw(),
            cq_base: registers.cq_base.raw(),
            cq_depth: registers.cq_depth.raw(),
            eq_base: registers.eq_base.raw(),
            eq_depth: registers.eq_depth.raw(),
            command: registers.command.raw(),
        }
    }

    pub(crate) fn complete(
        &mut self,
        result: MmioResultDetails,
        update: ResponseUpdate<QueueResponseBank>,
    ) {
        let registers = &mut self.registers;
        registers.response = apply_response_update(registers.response, update);
        registers.result = result.result.code().into();
        let [detail0, detail1, detail2] = result.details;
        registers.detail0 = detail0.into();
        registers.detail1 = detail1.into();
        registers.detail2 = detail2.into();
    }
}

pub(crate) fn status_response(
    status: crate::queue::exact::QueueStatus,
) -> Result<QueueResponseBank, MmioResultDetails> {
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

pub(crate) fn execute(state: &mut DeviceCore) -> bool {
    let request = state.queues.snapshot_request();
    let (result, response, changed) = queue_control::execute(state, request, request.command);
    state.queues.complete(result, response);
    changed
}
