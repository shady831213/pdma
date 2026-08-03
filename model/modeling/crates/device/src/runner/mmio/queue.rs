use super::bank::{CommandBank, MmioScalar, narrow_u32};
use crate::runner::state::RunnerState;
use pdma_model_contract::abi::mmio::{QueueResponseBank, ResponseUpdate};

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
pub(super) struct QueueBank {
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
}

pub(super) const fn read(state: &RunnerState, register: QueueRegister) -> MmioScalar {
    let bank = &state.mmio.queue;
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
        QueueRegister::RspEpoch => MmioScalar::U64(response.epoch),
        QueueRegister::RspId => MmioScalar::U64(response.queue_id),
        QueueRegister::RspSqProducer => MmioScalar::U64(response.sq_producer),
        QueueRegister::RspSqCaptured => MmioScalar::U64(response.sq_captured),
        QueueRegister::RspSqConsumer => MmioScalar::U64(response.sq_consumer),
        QueueRegister::RspCqProducer => MmioScalar::U64(response.cq_producer),
        QueueRegister::RspCqConsumer => MmioScalar::U64(response.cq_consumer),
        QueueRegister::RspEqProducer => MmioScalar::U64(response.eq_producer),
        QueueRegister::RspEqConsumer => MmioScalar::U64(response.eq_consumer),
    }
}

pub(super) fn write(state: &mut RunnerState, register: QueueRegister, value: u64) {
    let bank = &mut state.mmio.queue;
    match register {
        QueueRegister::ExpectedEpoch => bank.expected_epoch = value,
        QueueRegister::Id => bank.id = value,
        QueueRegister::ExpectedCursor => bank.expected_cursor = value,
        QueueRegister::NewCursor => bank.new_cursor = value,
        QueueRegister::CfgSqDepth => bank.cfg_sq_depth = narrow_u32(value),
        QueueRegister::CfgCqDepth => bank.cfg_cq_depth = narrow_u32(value),
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
        QueueRegister::CfgSoftwareOwner => bank.cfg_software_owner = value,
        QueueRegister::SqBase => bank.sq_base = value,
        QueueRegister::SqDepth => bank.sq_depth = narrow_u32(value),
        QueueRegister::CqBase => bank.cq_base = value,
        QueueRegister::CqDepth => bank.cq_depth = narrow_u32(value),
        QueueRegister::EqBase => bank.eq_base = value,
        QueueRegister::EqDepth => bank.eq_depth = narrow_u32(value),
        QueueRegister::Command => {
            let result = bank.command.result;
            bank.command.complete(result, ResponseUpdate::Preserve);
        }
    }
}
