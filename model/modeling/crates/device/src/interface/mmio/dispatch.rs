use super::{global, irq, queue};
use crate::device::DeviceCore;
use crate::register_file::{MmioScalar, narrow_u32};
use pdma_model_contract::abi::mmio::RegisterId;

pub(crate) const fn read(state: &DeviceCore, register: RegisterId) -> MmioScalar {
    let global = &state.registers.global;
    let global_response = global.command.response;
    let global_result = global.command.result;
    let [global_detail0, global_detail1, global_detail2] = global_result.details;
    let queue = &state.registers.queue;
    let queue_response = queue.command.response;
    let queue_result = queue.command.result;
    let [queue_detail0, queue_detail1, queue_detail2] = queue_result.details;
    let irq = &state.registers.irq;
    let irq_response = irq.command.response;
    let irq_result = irq.command.result;
    let [irq_detail0, irq_detail1, irq_detail2] = irq_result.details;
    match register {
        RegisterId::GMagic => MmioScalar::U32(0x414d_4450),
        RegisterId::GMmioAbiVersion => MmioScalar::U32(1),
        RegisterId::GReqAbiVersion => MmioScalar::U32(global.req_abi_version),
        RegisterId::GReqFeatures => MmioScalar::U64(global.req_features),
        RegisterId::GResetExpectedEpoch => MmioScalar::U64(global.reset_expected_epoch),
        RegisterId::GCommand
        | RegisterId::QCfgEventPolicy
        | RegisterId::QCommand
        | RegisterId::ICommand => MmioScalar::U32(0),
        RegisterId::GResult => MmioScalar::U32(global_result.result.code()),
        RegisterId::GDetail0 => MmioScalar::U64(global_detail0),
        RegisterId::GDetail1 => MmioScalar::U64(global_detail1),
        RegisterId::GDetail2 => MmioScalar::U64(global_detail2),
        RegisterId::GRspAbiVersion => MmioScalar::U32(global_response.abi_version.raw()),
        RegisterId::GRspRunState => MmioScalar::U32(global_response.run_state.raw()),
        RegisterId::GRspFeatures => MmioScalar::U64(global_response.features.raw()),
        RegisterId::GRspEpoch => MmioScalar::U64(global_response.epoch.raw()),
        RegisterId::GMaxRingDepth => MmioScalar::U32(global_response.max_ring_depth.raw()),
        RegisterId::GMaxIndirectDestinations => {
            MmioScalar::U32(global_response.max_indirect_destinations.raw())
        }
        RegisterId::GMaxTransferBytes => MmioScalar::U32(global_response.max_transfer_bytes.raw()),
        RegisterId::GMaxStridedRows => MmioScalar::U32(global_response.max_strided_rows.raw()),
        RegisterId::GMaxStride => MmioScalar::U32(global_response.max_stride.raw()),
        RegisterId::QExpectedEpoch => MmioScalar::U64(queue.expected_epoch),
        RegisterId::QId => MmioScalar::U64(queue.id),
        RegisterId::QExpectedCursor => MmioScalar::U64(queue.expected_cursor),
        RegisterId::QNewCursor => MmioScalar::U64(queue.new_cursor),
        RegisterId::QCfgSqDepth => MmioScalar::U32(queue.cfg_sq_depth),
        RegisterId::QCfgCqDepth => MmioScalar::U32(queue.cfg_cq_depth),
        RegisterId::QCfgSoftwareOwner => MmioScalar::U64(queue.cfg_software_owner),
        RegisterId::QSqBase => MmioScalar::U64(queue.sq_base),
        RegisterId::QSqDepth => MmioScalar::U32(queue.sq_depth),
        RegisterId::QCqBase => MmioScalar::U64(queue.cq_base),
        RegisterId::QCqDepth => MmioScalar::U32(queue.cq_depth),
        RegisterId::QEqBase => MmioScalar::U64(queue.eq_base),
        RegisterId::QEqDepth => MmioScalar::U32(queue.eq_depth),
        RegisterId::QResult => MmioScalar::U32(queue_result.result.code()),
        RegisterId::QDetail0 => MmioScalar::U64(queue_detail0),
        RegisterId::QDetail1 => MmioScalar::U64(queue_detail1),
        RegisterId::QDetail2 => MmioScalar::U64(queue_detail2),
        RegisterId::QRspEpoch => MmioScalar::U64(queue_response.epoch.raw()),
        RegisterId::QRspId => MmioScalar::U64(queue_response.queue_id.raw()),
        RegisterId::QRspSqProducer => MmioScalar::U64(queue_response.sq_producer.raw()),
        RegisterId::QRspSqCaptured => MmioScalar::U64(queue_response.sq_captured.raw()),
        RegisterId::QRspSqConsumer => MmioScalar::U64(queue_response.sq_consumer.raw()),
        RegisterId::QRspCqProducer => MmioScalar::U64(queue_response.cq_producer.raw()),
        RegisterId::QRspCqConsumer => MmioScalar::U64(queue_response.cq_consumer.raw()),
        RegisterId::QRspEqProducer => MmioScalar::U64(queue_response.eq_producer.raw()),
        RegisterId::QRspEqConsumer => MmioScalar::U64(queue_response.eq_consumer.raw()),
        RegisterId::IExpectedEpoch => MmioScalar::U64(irq.expected_epoch),
        RegisterId::ISourceId => MmioScalar::U64(irq.source_id),
        RegisterId::ISourceKind => MmioScalar::U32(irq.source_kind),
        RegisterId::IRouteKind => MmioScalar::U32(irq.route_kind),
        RegisterId::IRouteVector => MmioScalar::U64(irq.route_vector),
        RegisterId::IResult => MmioScalar::U32(irq_result.result.code()),
        RegisterId::IDetail0 => MmioScalar::U64(irq_detail0),
        RegisterId::IDetail1 => MmioScalar::U64(irq_detail1),
        RegisterId::IDetail2 => MmioScalar::U64(irq_detail2),
        RegisterId::IRspEpoch => MmioScalar::U64(irq_response.epoch.raw()),
        RegisterId::IRspSourceId => MmioScalar::U64(irq_response.source_id.raw()),
        RegisterId::IRspSourceKind => MmioScalar::U32(irq_response.source_kind.raw()),
        RegisterId::IRspState => MmioScalar::U32(irq_response.state.raw()),
        RegisterId::IRspRouteKind => MmioScalar::U32(irq_response.route_kind.raw()),
        RegisterId::IRspNotificationKind => MmioScalar::U32(irq_response.notification_kind.raw()),
        RegisterId::IRspVector => MmioScalar::U64(irq_response.vector.raw()),
        RegisterId::IRspMessageId => MmioScalar::U64(irq_response.message_id.raw()),
        RegisterId::IRspPendingMessages => MmioScalar::U64(irq_response.pending_messages.raw()),
    }
}

pub(crate) fn write(state: &mut DeviceCore, register: RegisterId, value: u64) -> bool {
    match register {
        RegisterId::GReqAbiVersion => {
            state.registers.global.req_abi_version = narrow_u32(value);
            false
        }
        RegisterId::GReqFeatures => {
            state.registers.global.req_features = value;
            false
        }
        RegisterId::GResetExpectedEpoch => {
            state.registers.global.reset_expected_epoch = value;
            false
        }
        RegisterId::GCommand => global::execute(state, narrow_u32(value)),
        RegisterId::QExpectedEpoch => set_u64(&mut state.registers.queue.expected_epoch, value),
        RegisterId::QId => set_u64(&mut state.registers.queue.id, value),
        RegisterId::QExpectedCursor => set_u64(&mut state.registers.queue.expected_cursor, value),
        RegisterId::QNewCursor => set_u64(&mut state.registers.queue.new_cursor, value),
        RegisterId::QCfgSqDepth => {
            set_u32(&mut state.registers.queue.cfg_sq_depth, narrow_u32(value))
        }
        RegisterId::QCfgCqDepth => {
            set_u32(&mut state.registers.queue.cfg_cq_depth, narrow_u32(value))
        }
        RegisterId::QCfgSoftwareOwner => {
            set_u64(&mut state.registers.queue.cfg_software_owner, value)
        }
        RegisterId::QSqBase => set_u64(&mut state.registers.queue.sq_base, value),
        RegisterId::QSqDepth => set_u32(&mut state.registers.queue.sq_depth, narrow_u32(value)),
        RegisterId::QCqBase => set_u64(&mut state.registers.queue.cq_base, value),
        RegisterId::QCqDepth => set_u32(&mut state.registers.queue.cq_depth, narrow_u32(value)),
        RegisterId::QEqBase => set_u64(&mut state.registers.queue.eq_base, value),
        RegisterId::QEqDepth => set_u32(&mut state.registers.queue.eq_depth, narrow_u32(value)),
        RegisterId::QCommand => queue::execute(state, value),
        RegisterId::IExpectedEpoch => set_u64(&mut state.registers.irq.expected_epoch, value),
        RegisterId::ISourceId => set_u64(&mut state.registers.irq.source_id, value),
        RegisterId::ISourceKind => set_u32(&mut state.registers.irq.source_kind, narrow_u32(value)),
        RegisterId::IRouteKind => set_u32(&mut state.registers.irq.route_kind, narrow_u32(value)),
        RegisterId::IRouteVector => set_u64(&mut state.registers.irq.route_vector, value),
        RegisterId::ICommand => irq::execute(state, narrow_u32(value)),
        RegisterId::GMagic
        | RegisterId::GMmioAbiVersion
        | RegisterId::GResult
        | RegisterId::GDetail0
        | RegisterId::GDetail1
        | RegisterId::GDetail2
        | RegisterId::GRspAbiVersion
        | RegisterId::GRspRunState
        | RegisterId::GRspFeatures
        | RegisterId::GRspEpoch
        | RegisterId::GMaxRingDepth
        | RegisterId::GMaxIndirectDestinations
        | RegisterId::GMaxTransferBytes
        | RegisterId::GMaxStridedRows
        | RegisterId::GMaxStride
        | RegisterId::QCfgEventPolicy
        | RegisterId::QResult
        | RegisterId::QDetail0
        | RegisterId::QDetail1
        | RegisterId::QDetail2
        | RegisterId::QRspEpoch
        | RegisterId::QRspId
        | RegisterId::QRspSqProducer
        | RegisterId::QRspSqCaptured
        | RegisterId::QRspSqConsumer
        | RegisterId::QRspCqProducer
        | RegisterId::QRspCqConsumer
        | RegisterId::QRspEqProducer
        | RegisterId::QRspEqConsumer
        | RegisterId::IResult
        | RegisterId::IDetail0
        | RegisterId::IDetail1
        | RegisterId::IDetail2
        | RegisterId::IRspEpoch
        | RegisterId::IRspSourceId
        | RegisterId::IRspSourceKind
        | RegisterId::IRspState
        | RegisterId::IRspRouteKind
        | RegisterId::IRspNotificationKind
        | RegisterId::IRspVector
        | RegisterId::IRspMessageId
        | RegisterId::IRspPendingMessages => false,
    }
}

const fn set_u32(target: &mut u32, value: u32) -> bool {
    *target = value;
    false
}

const fn set_u64(target: &mut u64, value: u64) -> bool {
    *target = value;
    false
}
