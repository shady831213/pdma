use super::bank::{CommandBank, MmioScalar, narrow_u32};
use crate::runner::native::probe::{RunStateProbe, probe_run_state};
use crate::runner::state::DeviceCore;
use pdma_model_contract::abi::mmio::{
    GlobalCommand, GlobalResponseBank, MmioResult, MmioResultDetails, ResponseUpdate,
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

#[derive(Debug)]
pub(super) struct GlobalBank {
    req_abi_version: u32,
    req_features: u64,
    reset_expected_epoch: u64,
    command: CommandBank<GlobalResponseBank>,
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
        GlobalRegister::RspAbiVersion => MmioScalar::U32(response.abi_version.raw()),
        GlobalRegister::RspRunState => MmioScalar::U32(response.run_state.raw()),
        GlobalRegister::RspFeatures => MmioScalar::U64(response.features.raw()),
        GlobalRegister::RspEpoch => MmioScalar::U64(response.epoch.raw()),
        GlobalRegister::MaxRingDepth => MmioScalar::U32(response.max_ring_depth.raw()),
        GlobalRegister::MaxIndirectDestinations => {
            MmioScalar::U32(response.max_indirect_destinations.raw())
        }
        GlobalRegister::MaxTransferBytes => MmioScalar::U32(response.max_transfer_bytes.raw()),
        GlobalRegister::MaxStridedRows => MmioScalar::U32(response.max_strided_rows.raw()),
        GlobalRegister::MaxStride => MmioScalar::U32(response.max_stride.raw()),
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
                GlobalResponseBank {
                    epoch: epoch.raw().into(),
                    ..GlobalResponseBank::default()
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
                GlobalResponseBank {
                    abi_version: u32::from(discovery.version()).into(),
                    features: discovery.features().bits().into(),
                    ..GlobalResponseBank::default()
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
        GlobalResponseBank {
            max_ring_depth: limits.max_ring_depth().into(),
            max_indirect_destinations: limits.max_indirect_destinations().into(),
            max_transfer_bytes: limits.max_transfer_bytes().into(),
            max_strided_rows: limits.max_strided_rows().into(),
            max_stride: limits.max_stride().into(),
            ..GlobalResponseBank::default()
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
        GlobalResponseBank {
            run_state: encoded.into(),
            epoch: state.backend.epoch().raw().into(),
            ..GlobalResponseBank::default()
        },
    );
}

fn complete_success(state: &mut DeviceCore, response: GlobalResponseBank) {
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
