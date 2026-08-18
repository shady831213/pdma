use super::bank::{CommandBank, MmioScalar, narrow_u32};
use crate::runner::native::probe::{RunStateProbe, probe_run_state};
use crate::runner::state::DeviceCore;
use pdma_model_contract::abi::mmio::{
    GlobalCommand, MmioResult, MmioResultDetails, ResponseUpdate,
};
use pdma_model_contract::bridge::{AbiLimits, DiscoveryRequest};
use pdma_model_contract::semantic::DeviceRunState;

#[path = "global/error.rs"]
mod error;

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

pub(super) const fn read(state: &DeviceCore, register: GlobalRegister) -> MmioScalar {
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

pub(super) fn write(state: &mut DeviceCore, register: GlobalRegister, value: u64) {
    let bank = &mut state.mmio.global;
    match register {
        GlobalRegister::ReqAbiVersion => bank.req_abi_version = narrow_u32(value),
        GlobalRegister::ReqFeatures => bank.req_features = value,
        GlobalRegister::ResetExpectedEpoch => bank.reset_expected_epoch = value,
        GlobalRegister::Command => {
            execute(state, narrow_u32(value));
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

fn execute(state: &mut DeviceCore, raw: u32) {
    let Ok(command) = GlobalCommand::try_from(raw) else {
        complete_failure(
            state,
            details(MmioResult::UnknownCommand, [u64::from(raw), 0, 0]),
        );
        return;
    };
    match command {
        GlobalCommand::Discover => discover(state),
        GlobalCommand::ReadLimits => read_limits(state),
        GlobalCommand::ReadStatus => read_status(state),
        GlobalCommand::Reset => reset(state),
    }
}

fn reset(state: &mut DeviceCore) {
    let expected_epoch = state.mmio.global.reset_expected_epoch;
    match super::reset::execute(state, expected_epoch) {
        super::reset::ResetOutcome::Success(epoch) => {
            complete_success(
                state,
                GlobalResponseRegisters {
                    epoch: epoch.raw(),
                    ..GlobalResponseRegisters::default()
                },
            );
            crate::runner::state::wake(state);
        }
        super::reset::ResetOutcome::Rejected(_, result) => complete_failure(state, result),
    }
}

fn discover(state: &mut DeviceCore) {
    let request = {
        let bank = &state.mmio.global;
        DiscoveryRequest::try_new(low_u16(bank.req_abi_version), bank.req_features)
    };
    match request {
        Ok(request) => {
            let discovery = crate::runner::global_api::discover(request);
            complete_success(
                state,
                GlobalResponseRegisters {
                    abi_version: u32::from(discovery.version()),
                    features: discovery.features().bits(),
                    ..GlobalResponseRegisters::default()
                },
            );
        }
        Err(error) => complete_failure(state, error::map_discovery(error)),
    }
}

fn read_limits(state: &mut DeviceCore) {
    let limits = AbiLimits::v1();
    complete_success(
        state,
        GlobalResponseRegisters {
            max_ring_depth: limits.max_ring_depth(),
            max_indirect_destinations: limits.max_indirect_destinations(),
            max_transfer_bytes: limits.max_transfer_bytes(),
            max_strided_rows: limits.max_strided_rows(),
            max_stride: limits.max_stride(),
            ..GlobalResponseRegisters::default()
        },
    );
}

fn read_status(state: &mut DeviceCore) {
    let run_state = match &state.backend {
        crate::runner::backend::DeviceModel::QueuePort(device) => {
            probe_run_state(device, &state.dma_space, &state.bus_control)
        }
    };
    let run_state = match run_state {
        RunStateProbe::Idle => DeviceRunState::Idle,
        RunStateProbe::Running => DeviceRunState::Running,
        RunStateProbe::Blocked { .. } => DeviceRunState::Blocked,
    };
    let encoded = match run_state {
        DeviceRunState::Idle => 0,
        DeviceRunState::Running => 1,
        DeviceRunState::Blocked => 2,
    };
    complete_success(
        state,
        GlobalResponseRegisters {
            run_state: encoded,
            epoch: state.backend.epoch().raw(),
            ..GlobalResponseRegisters::default()
        },
    );
}

fn complete_success(state: &mut DeviceCore, response: GlobalResponseRegisters) {
    state.mmio.global.command.complete(
        details(MmioResult::Ok, [0; 3]),
        ResponseUpdate::Replace(response),
    );
}

fn complete_failure(state: &mut DeviceCore, result: MmioResultDetails) {
    state
        .mmio
        .global
        .command
        .complete(result, ResponseUpdate::Preserve);
}

const fn details(result: MmioResult, values: [u64; 3]) -> MmioResultDetails {
    MmioResultDetails {
        result,
        details: values,
    }
}

fn low_u16(value: u32) -> u16 {
    u16::try_from(value).unwrap_or_default()
}
