use crate::device::DeviceCore;
use crate::register_file::{PdmaRegisters, narrow_u32};
use pdma_model_contract::abi::mmio::{
    MmioResultDetails, PdmaMmioMapViewMut, QueueResponseBank, ResponseUpdate, apply_response_update,
};

mod queue_config;
pub(crate) mod queue_control;
pub(crate) use queue_control::map_bus as map_bus_error;

#[derive(Clone, Copy)]
pub(crate) struct QueueRequest {
    pub(crate) expected_epoch: u64,
    pub(crate) id: u64,
    pub(crate) expected_cursor: u64,
    pub(crate) new_cursor: u64,
    pub(crate) cfg_sq_depth: u32,
    pub(crate) cfg_cq_depth: u32,
    pub(crate) cfg_event_policy: u32,
    pub(crate) cfg_software_owner: u64,
    pub(crate) sq_base: u64,
    pub(crate) sq_depth: u32,
    pub(crate) cq_base: u64,
    pub(crate) cq_depth: u32,
    pub(crate) eq_base: u64,
    pub(crate) eq_depth: u32,
    pub(crate) command: u32,
}

pub(crate) fn snapshot_request(registers: &PdmaRegisters) -> QueueRequest {
    registers.view(|root| {
        let queue = invariant(root.queue());
        QueueRequest {
            expected_epoch: invariant(invariant(queue.expected_epoch()).value()),
            id: invariant(invariant(queue.id()).value()),
            expected_cursor: invariant(invariant(queue.expected_cursor()).value()),
            new_cursor: invariant(invariant(queue.new_cursor()).value()),
            cfg_sq_depth: narrow_u32(invariant(invariant(queue.cfg_sq_depth()).value())),
            cfg_cq_depth: narrow_u32(invariant(invariant(queue.cfg_cq_depth()).value())),
            cfg_event_policy: narrow_u32(invariant(invariant(queue.cfg_event_policy()).value())),
            cfg_software_owner: invariant(invariant(queue.cfg_software_owner()).value()),
            sq_base: invariant(invariant(queue.sq_base()).value()),
            sq_depth: narrow_u32(invariant(invariant(queue.sq_depth()).value())),
            cq_base: invariant(invariant(queue.cq_base()).value()),
            cq_depth: narrow_u32(invariant(invariant(queue.cq_depth()).value())),
            eq_base: invariant(invariant(queue.eq_base()).value()),
            eq_depth: narrow_u32(invariant(invariant(queue.eq_depth()).value())),
            command: narrow_u32(invariant(invariant(queue.command()).value())),
        }
    })
}

pub(crate) fn install_request(root: &mut PdmaMmioMapViewMut<'_>, request: QueueRequest) {
    let mut queue = invariant(root.queue_mut());
    invariant(invariant(queue.expected_epoch_mut()).set_value(request.expected_epoch));
    invariant(invariant(queue.id_mut()).set_value(request.id));
    invariant(invariant(queue.expected_cursor_mut()).set_value(request.expected_cursor));
    invariant(invariant(queue.new_cursor_mut()).set_value(request.new_cursor));
    invariant(invariant(queue.cfg_sq_depth_mut()).set_value(u64::from(request.cfg_sq_depth)));
    invariant(invariant(queue.cfg_cq_depth_mut()).set_value(u64::from(request.cfg_cq_depth)));
    invariant(
        invariant(queue.cfg_event_policy_mut()).set_value(u64::from(request.cfg_event_policy)),
    );
    invariant(invariant(queue.cfg_software_owner_mut()).set_value(request.cfg_software_owner));
    invariant(invariant(queue.sq_base_mut()).set_value(request.sq_base));
    invariant(invariant(queue.sq_depth_mut()).set_value(u64::from(request.sq_depth)));
    invariant(invariant(queue.cq_base_mut()).set_value(request.cq_base));
    invariant(invariant(queue.cq_depth_mut()).set_value(u64::from(request.cq_depth)));
    invariant(invariant(queue.eq_base_mut()).set_value(request.eq_base));
    invariant(invariant(queue.eq_depth_mut()).set_value(u64::from(request.eq_depth)));
    invariant(invariant(queue.command_mut()).set_value(u64::from(request.command)));
}

pub(crate) fn complete(
    root: &mut PdmaMmioMapViewMut<'_>,
    result: MmioResultDetails,
    update: ResponseUpdate<QueueResponseBank>,
) {
    let mut queue = invariant(root.queue_mut());
    let previous = QueueResponseBank {
        epoch: invariant(invariant(queue.rsp_epoch()).value()).into(),
        queue_id: invariant(invariant(queue.rsp_id()).value()).into(),
        sq_producer: invariant(invariant(queue.rsp_sq_producer()).value()).into(),
        sq_captured: invariant(invariant(queue.rsp_sq_captured()).value()).into(),
        sq_consumer: invariant(invariant(queue.rsp_sq_consumer()).value()).into(),
        cq_producer: invariant(invariant(queue.rsp_cq_producer()).value()).into(),
        cq_consumer: invariant(invariant(queue.rsp_cq_consumer()).value()).into(),
        eq_producer: invariant(invariant(queue.rsp_eq_producer()).value()).into(),
        eq_consumer: invariant(invariant(queue.rsp_eq_consumer()).value()).into(),
    };
    let response = apply_response_update(previous, update);
    invariant(invariant(queue.result_mut()).set_value(u64::from(result.result.code())));
    let [detail0, detail1, detail2] = result.details;
    invariant(invariant(queue.detail0_mut()).set_value(detail0));
    invariant(invariant(queue.detail1_mut()).set_value(detail1));
    invariant(invariant(queue.detail2_mut()).set_value(detail2));
    invariant(invariant(queue.rsp_epoch_mut()).set_value(response.epoch.raw()));
    invariant(invariant(queue.rsp_id_mut()).set_value(response.queue_id.raw()));
    invariant(invariant(queue.rsp_sq_producer_mut()).set_value(response.sq_producer.raw()));
    invariant(invariant(queue.rsp_sq_captured_mut()).set_value(response.sq_captured.raw()));
    invariant(invariant(queue.rsp_sq_consumer_mut()).set_value(response.sq_consumer.raw()));
    invariant(invariant(queue.rsp_cq_producer_mut()).set_value(response.cq_producer.raw()));
    invariant(invariant(queue.rsp_cq_consumer_mut()).set_value(response.cq_consumer.raw()));
    invariant(invariant(queue.rsp_eq_producer_mut()).set_value(response.eq_producer.raw()));
    invariant(invariant(queue.rsp_eq_consumer_mut()).set_value(response.eq_consumer.raw()));
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

pub(super) fn execute(state: &mut DeviceCore, _value: u64) -> bool {
    let request = snapshot_request(&state.registers);
    let (result, response, changed) = queue_control::execute(state, request, request.command);
    state
        .registers
        .view_mut(|mut root| complete(&mut root, result, response));
    changed
}

fn invariant<T, E>(result: Result<T, E>) -> T {
    let Ok(value) = result else {
        std::process::abort();
    };
    value
}
