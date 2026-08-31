//! Queue MMIO register state and command dispatch.

use super::bank::{CommandBank, MmioScalar, narrow_u32};
use crate::runtime::state::DeviceCore;
use pdma_model_contract::abi::mmio::QueueResponseBank;

pub mod batch;
mod queue_config;
pub mod queue_control;
pub(crate) use batch::{BootstrapBatchStaging, SubmissionBatchStaging};
pub(crate) use queue_control::map_bus as map_bus_error;

#[derive(Clone, Copy)]
pub(super) enum QueueRegister {
    ExpectedEpoch,
    Id,
    ExpectedCursor,
    NewCursor,
    CfgSqDepth,
    CfgCqDepth,
    CfgEventPolicy,
    CfgSoftwareOwner,
    SqBase,
    SqDepth,
    CqBase,
    CqDepth,
    EqBase,
    EqDepth,
    Command,
    Result,
    Detail0,
    Detail1,
    Detail2,
    RspEpoch,
    RspId,
    RspSqProducer,
    RspSqCaptured,
    RspSqConsumer,
    RspCqProducer,
    RspCqConsumer,
    RspEqProducer,
    RspEqConsumer,
}

#[derive(Debug)]
pub(crate) struct QueueBank {
    expected_epoch: u64,
    id: u64,
    expected_cursor: u64,
    new_cursor: u64,
    cfg_sq_depth: u32,
    cfg_cq_depth: u32,
    cfg_software_owner: u64,
    sq_base: u64,
    sq_depth: u32,
    cq_base: u64,
    cq_depth: u32,
    eq_base: u64,
    eq_depth: u32,
    command: CommandBank<QueueResponseBank>,
}

#[derive(Clone, Copy)]
pub(super) struct QueueStaging {
    expected_epoch: u64,
    id: u64,
    expected_cursor: u64,
    new_cursor: u64,
    cfg_sq_depth: u32,
    cfg_cq_depth: u32,
    cfg_software_owner: u64,
    sq_base: u64,
    sq_depth: u32,
    cq_base: u64,
    cq_depth: u32,
    eq_base: u64,
    eq_depth: u32,
}

impl QueueBank {
    pub(super) fn new() -> Self {
        Self {
            expected_epoch: 0,
            id: 0,
            expected_cursor: 0,
            new_cursor: 0,
            cfg_sq_depth: 0,
            cfg_cq_depth: 0,
            cfg_software_owner: 0,
            sq_base: 0,
            sq_depth: 0,
            cq_base: 0,
            cq_depth: 0,
            eq_base: 0,
            eq_depth: 0,
            command: CommandBank::new(),
        }
    }

    const fn staging(&self) -> QueueStaging {
        QueueStaging {
            expected_epoch: self.expected_epoch,
            id: self.id,
            expected_cursor: self.expected_cursor,
            new_cursor: self.new_cursor,
            cfg_sq_depth: self.cfg_sq_depth,
            cfg_cq_depth: self.cfg_cq_depth,
            cfg_software_owner: self.cfg_software_owner,
            sq_base: self.sq_base,
            sq_depth: self.sq_depth,
            cq_base: self.cq_base,
            cq_depth: self.cq_depth,
            eq_base: self.eq_base,
            eq_depth: self.eq_depth,
        }
    }
}

pub(super) const fn read(state: &DeviceCore, register: QueueRegister) -> MmioScalar {
    let bank = &state.device.registers.queue;
    let response = bank.command.response;
    let result = bank.command.result;
    let [detail0, detail1, detail2] = result.details;
    match register {
        QueueRegister::ExpectedEpoch => MmioScalar::U64(bank.expected_epoch),
        QueueRegister::Id => MmioScalar::U64(bank.id),
        QueueRegister::ExpectedCursor => MmioScalar::U64(bank.expected_cursor),
        QueueRegister::NewCursor => MmioScalar::U64(bank.new_cursor),
        QueueRegister::CfgSqDepth => MmioScalar::U32(bank.cfg_sq_depth),
        QueueRegister::CfgCqDepth => MmioScalar::U32(bank.cfg_cq_depth),
        QueueRegister::CfgEventPolicy | QueueRegister::Command => MmioScalar::U32(0),
        QueueRegister::CfgSoftwareOwner => MmioScalar::U64(bank.cfg_software_owner),
        QueueRegister::SqBase => MmioScalar::U64(bank.sq_base),
        QueueRegister::SqDepth => MmioScalar::U32(bank.sq_depth),
        QueueRegister::CqBase => MmioScalar::U64(bank.cq_base),
        QueueRegister::CqDepth => MmioScalar::U32(bank.cq_depth),
        QueueRegister::EqBase => MmioScalar::U64(bank.eq_base),
        QueueRegister::EqDepth => MmioScalar::U32(bank.eq_depth),
        QueueRegister::Result => MmioScalar::U32(result.result.code()),
        QueueRegister::Detail0 => MmioScalar::U64(detail0),
        QueueRegister::Detail1 => MmioScalar::U64(detail1),
        QueueRegister::Detail2 => MmioScalar::U64(detail2),
        QueueRegister::RspEpoch => MmioScalar::U64(response.epoch.raw()),
        QueueRegister::RspId => MmioScalar::U64(response.queue_id.raw()),
        QueueRegister::RspSqProducer => MmioScalar::U64(response.sq_producer.raw()),
        QueueRegister::RspSqCaptured => MmioScalar::U64(response.sq_captured.raw()),
        QueueRegister::RspSqConsumer => MmioScalar::U64(response.sq_consumer.raw()),
        QueueRegister::RspCqProducer => MmioScalar::U64(response.cq_producer.raw()),
        QueueRegister::RspCqConsumer => MmioScalar::U64(response.cq_consumer.raw()),
        QueueRegister::RspEqProducer => MmioScalar::U64(response.eq_producer.raw()),
        QueueRegister::RspEqConsumer => MmioScalar::U64(response.eq_consumer.raw()),
    }
}

pub(crate) fn status_response(
    status: crate::runtime::QueueStatus,
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

pub(super) fn write(state: &mut DeviceCore, register: QueueRegister, value: u64) {
    match register {
        QueueRegister::ExpectedEpoch => state.device.registers.queue.expected_epoch = value,
        QueueRegister::Id => state.device.registers.queue.id = value,
        QueueRegister::ExpectedCursor => state.device.registers.queue.expected_cursor = value,
        QueueRegister::NewCursor => state.device.registers.queue.new_cursor = value,
        QueueRegister::CfgSqDepth => state.device.registers.queue.cfg_sq_depth = narrow_u32(value),
        QueueRegister::CfgCqDepth => state.device.registers.queue.cfg_cq_depth = narrow_u32(value),
        QueueRegister::CfgEventPolicy
        | QueueRegister::Result
        | QueueRegister::Detail0
        | QueueRegister::Detail1
        | QueueRegister::Detail2
        | QueueRegister::RspEpoch
        | QueueRegister::RspId
        | QueueRegister::RspSqProducer
        | QueueRegister::RspSqCaptured
        | QueueRegister::RspSqConsumer
        | QueueRegister::RspCqProducer
        | QueueRegister::RspCqConsumer
        | QueueRegister::RspEqProducer
        | QueueRegister::RspEqConsumer => {}
        QueueRegister::CfgSoftwareOwner => state.device.registers.queue.cfg_software_owner = value,
        QueueRegister::SqBase => state.device.registers.queue.sq_base = value,
        QueueRegister::SqDepth => state.device.registers.queue.sq_depth = narrow_u32(value),
        QueueRegister::CqBase => state.device.registers.queue.cq_base = value,
        QueueRegister::CqDepth => state.device.registers.queue.cq_depth = narrow_u32(value),
        QueueRegister::EqBase => state.device.registers.queue.eq_base = value,
        QueueRegister::EqDepth => state.device.registers.queue.eq_depth = narrow_u32(value),
        QueueRegister::Command => {
            let staging = state.device.registers.queue.staging();
            let (result, response, changed) = queue_control::execute(state, staging, value);
            state
                .device
                .registers
                .queue
                .command
                .complete(result, response);
            if changed {
                crate::runtime::state::wake(state);
            }
        }
    }
}
