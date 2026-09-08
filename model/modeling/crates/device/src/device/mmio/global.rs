use super::DeviceCore;
use pdma_model_contract::abi::mmio::{
    GlobalCommand, GlobalRegistersValues, GlobalResponseBank, MmioResult, MmioResultDetails,
    ResponseUpdate, apply_response_update,
};
use pdma_model_contract::abi::{AbiLimits, DiscoveryRequest};
use pdma_model_contract::semantic::DeviceRunState;

mod error;

pub(super) fn execute(state: &mut DeviceCore, raw: u32) -> bool {
    let request = snapshot_request(&state.global_registers);
    let Ok(command) = GlobalCommand::try_from(raw) else {
        complete_failure(
            state,
            details(MmioResult::UnknownCommand, [u64::from(raw), 0, 0]),
        );
        return false;
    };
    match command {
        GlobalCommand::Discover => {
            discover(state, request);
            false
        }
        GlobalCommand::ReadLimits => {
            read_limits(state);
            false
        }
        GlobalCommand::ReadStatus => {
            read_status(state);
            false
        }
        GlobalCommand::Reset => reset(state, request),
    }
}

#[derive(Clone, Copy)]
struct GlobalRequest {
    abi_version: u32,
    features: u64,
    reset_expected_epoch: u64,
}

const fn snapshot_request(registers: &GlobalRegistersValues) -> GlobalRequest {
    GlobalRequest {
        abi_version: registers.req_abi_version.raw(),
        features: registers.req_features.raw(),
        reset_expected_epoch: registers.reset_expected_epoch.raw(),
    }
}

fn reset(state: &mut DeviceCore, request: GlobalRequest) -> bool {
    match super::reset::execute(state, request.reset_expected_epoch) {
        super::reset::ResetOutcome::Success(epoch) => {
            complete_success(
                state,
                GlobalResponseBank {
                    epoch: epoch.raw().into(),
                    ..GlobalResponseBank::default()
                },
            );
            true
        }
        super::reset::ResetOutcome::Rejected(_, result) => {
            complete_failure(state, result);
            false
        }
    }
}

fn discover(state: &mut DeviceCore, request: GlobalRequest) {
    match DiscoveryRequest::try_new(low_u16(request.abi_version), request.features) {
        Ok(_request) => {
            let discovery = pdma_model_contract::abi::AbiDiscovery::v1();
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
    let run_state = state.run_state();
    let epoch = state.epoch();
    let encoded = match run_state {
        DeviceRunState::Idle => 0,
        DeviceRunState::Running => 1,
        DeviceRunState::Blocked => 2,
    };
    complete_success(
        state,
        GlobalResponseBank {
            run_state: encoded.into(),
            epoch: epoch.raw().into(),
            ..GlobalResponseBank::default()
        },
    );
}

fn complete_success(state: &mut DeviceCore, response: GlobalResponseBank) {
    complete(
        &mut state.global_registers,
        details(MmioResult::Ok, [0; 3]),
        ResponseUpdate::Replace(response),
    );
}

fn complete_failure(state: &mut DeviceCore, result: MmioResultDetails) {
    complete(
        &mut state.global_registers,
        result,
        ResponseUpdate::Preserve,
    );
}

fn complete(
    registers: &mut GlobalRegistersValues,
    result: MmioResultDetails,
    update: ResponseUpdate<GlobalResponseBank>,
) {
    registers.response = apply_response_update(registers.response, update);
    registers.result = result.result.code().into();
    let [detail0, detail1, detail2] = result.details;
    registers.detail0 = detail0.into();
    registers.detail1 = detail1.into();
    registers.detail2 = detail2.into();
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
