use super::bank::{CommandBank, MmioScalar, narrow_u32};
use crate::runner::state::RunnerState;
use pdma_model_contract::abi::mmio::ResponseUpdate;

#[derive(Clone, Copy)]
pub(super) enum GlobalRegister {
    Magic,
    MmioAbiVersion,
    ReqAbiVersion,
    ReqFeatures,
    ResetExpectedEpoch,
    Command,
    Result,
    Detail0,
    Detail1,
    Detail2,
    RspAbiVersion,
    RspRunState,
    RspFeatures,
    RspEpoch,
    MaxRingDepth,
    MaxIndirectDestinations,
    MaxTransferBytes,
    MaxStridedRows,
    MaxStride,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) struct GlobalResponseRegisters {
    abi_version: u32,
    run_state: u32,
    features: u64,
    epoch: u64,
    max_ring_depth: u32,
    max_indirect_destinations: u32,
    max_transfer_bytes: u32,
    max_strided_rows: u32,
    max_stride: u32,
}

#[derive(Debug)]
pub(super) struct GlobalBank {
    req_abi_version: u32,
    req_features: u64,
    reset_expected_epoch: u64,
    command: CommandBank<GlobalResponseRegisters>,
}

impl GlobalBank {
    pub(super) fn new() -> Self {
        Self {
            req_abi_version: 0,
            req_features: 0,
            reset_expected_epoch: 0,
            command: CommandBank::new(),
        }
    }
}

pub(super) const fn read(state: &RunnerState, register: GlobalRegister) -> MmioScalar {
    let bank = &state.mmio.global;
    let response = bank.command.response;
    let result = bank.command.result;
    let [detail0, detail1, detail2] = result.details;
    match register {
        GlobalRegister::Magic => MmioScalar::U32(0x414d_4450),
        GlobalRegister::MmioAbiVersion => MmioScalar::U32(1),
        GlobalRegister::ReqAbiVersion => MmioScalar::U32(bank.req_abi_version),
        GlobalRegister::ReqFeatures => MmioScalar::U64(bank.req_features),
        GlobalRegister::ResetExpectedEpoch => MmioScalar::U64(bank.reset_expected_epoch),
        GlobalRegister::Command => MmioScalar::U32(0),
        GlobalRegister::Result => MmioScalar::U32(result.result.code()),
        GlobalRegister::Detail0 => MmioScalar::U64(detail0),
        GlobalRegister::Detail1 => MmioScalar::U64(detail1),
        GlobalRegister::Detail2 => MmioScalar::U64(detail2),
        GlobalRegister::RspAbiVersion => MmioScalar::U32(response.abi_version),
        GlobalRegister::RspRunState => MmioScalar::U32(response.run_state),
        GlobalRegister::RspFeatures => MmioScalar::U64(response.features),
        GlobalRegister::RspEpoch => MmioScalar::U64(response.epoch),
        GlobalRegister::MaxRingDepth => MmioScalar::U32(response.max_ring_depth),
        GlobalRegister::MaxIndirectDestinations => {
            MmioScalar::U32(response.max_indirect_destinations)
        }
        GlobalRegister::MaxTransferBytes => MmioScalar::U32(response.max_transfer_bytes),
        GlobalRegister::MaxStridedRows => MmioScalar::U32(response.max_strided_rows),
        GlobalRegister::MaxStride => MmioScalar::U32(response.max_stride),
    }
}

pub(super) fn write(state: &mut RunnerState, register: GlobalRegister, value: u64) {
    let bank = &mut state.mmio.global;
    match register {
        GlobalRegister::ReqAbiVersion => bank.req_abi_version = narrow_u32(value),
        GlobalRegister::ReqFeatures => bank.req_features = value,
        GlobalRegister::ResetExpectedEpoch => bank.reset_expected_epoch = value,
        GlobalRegister::Command => {
            let result = bank.command.result;
            bank.command.complete(result, ResponseUpdate::Preserve);
        }
        GlobalRegister::Magic
        | GlobalRegister::MmioAbiVersion
        | GlobalRegister::Result
        | GlobalRegister::Detail0
        | GlobalRegister::Detail1
        | GlobalRegister::Detail2
        | GlobalRegister::RspAbiVersion
        | GlobalRegister::RspRunState
        | GlobalRegister::RspFeatures
        | GlobalRegister::RspEpoch
        | GlobalRegister::MaxRingDepth
        | GlobalRegister::MaxIndirectDestinations
        | GlobalRegister::MaxTransferBytes
        | GlobalRegister::MaxStridedRows
        | GlobalRegister::MaxStride => {}
    }
}
