use crate::device::DeviceCore;
use crate::register_file::PdmaRegisters;
use pdma_model_contract::abi::mmio::{
    GlobalCommand, GlobalResponseBank, MmioResult, MmioResultDetails, ResponseUpdate,
    apply_response_update,
};
use pdma_model_contract::abi::{AbiLimits, DiscoveryRequest};
use pdma_model_contract::semantic::DeviceRunState;

mod error;

pub(super) fn execute(state: &mut DeviceCore, raw: u32) -> bool {
    let request = snapshot_request(&state.registers, raw);
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

fn snapshot_request(registers: &PdmaRegisters, command: u32) -> GlobalRequest {
    registers.view_mut(|mut root| {
        let mut global = invariant(root.global_mut());
        invariant(invariant(global.command_mut()).set_value(u64::from(command)));
        GlobalRequest {
            abi_version: low_u32(invariant(invariant(global.req_abi_version()).value())),
            features: invariant(invariant(global.req_features()).value()),
            reset_expected_epoch: invariant(invariant(global.reset_expected_epoch()).value()),
        }
    })
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

fn discover(state: &DeviceCore, request: GlobalRequest) {
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

fn read_limits(state: &DeviceCore) {
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

fn read_status(state: &DeviceCore) {
    let run_state = state.run_state();
    let encoded = match run_state {
        DeviceRunState::Idle => 0,
        DeviceRunState::Running => 1,
        DeviceRunState::Blocked => 2,
    };
    complete_success(
        state,
        GlobalResponseBank {
            run_state: encoded.into(),
            epoch: state.epoch().raw().into(),
            ..GlobalResponseBank::default()
        },
    );
}

fn complete_success(state: &DeviceCore, response: GlobalResponseBank) {
    complete(
        &state.registers,
        details(MmioResult::Ok, [0; 3]),
        ResponseUpdate::Replace(response),
    );
}

fn complete_failure(state: &DeviceCore, result: MmioResultDetails) {
    complete(&state.registers, result, ResponseUpdate::Preserve);
}

fn complete(
    registers: &PdmaRegisters,
    result: MmioResultDetails,
    update: ResponseUpdate<GlobalResponseBank>,
) {
    registers.view_mut(|mut root| {
        let mut global = invariant(root.global_mut());
        let previous = GlobalResponseBank {
            abi_version: low_u32(invariant(invariant(global.rsp_abi_version()).value())).into(),
            run_state: low_u32(invariant(invariant(global.rsp_run_state()).value())).into(),
            features: invariant(invariant(global.rsp_features()).value()).into(),
            epoch: invariant(invariant(global.rsp_epoch()).value()).into(),
            max_ring_depth: low_u32(invariant(invariant(global.max_ring_depth()).value())).into(),
            max_indirect_destinations: low_u32(invariant(
                invariant(global.max_indirect_destinations()).value(),
            ))
            .into(),
            max_transfer_bytes: low_u32(invariant(invariant(global.max_transfer_bytes()).value()))
                .into(),
            max_strided_rows: low_u32(invariant(invariant(global.max_strided_rows()).value()))
                .into(),
            max_stride: low_u32(invariant(invariant(global.max_stride()).value())).into(),
        };
        let response = apply_response_update(previous, update);
        invariant(invariant(global.result_mut()).set_value(u64::from(result.result.code())));
        let [detail0, detail1, detail2] = result.details;
        invariant(invariant(global.detail0_mut()).set_value(detail0));
        invariant(invariant(global.detail1_mut()).set_value(detail1));
        invariant(invariant(global.detail2_mut()).set_value(detail2));
        invariant(
            invariant(global.rsp_abi_version_mut())
                .set_value(u64::from(response.abi_version.raw())),
        );
        invariant(
            invariant(global.rsp_run_state_mut()).set_value(u64::from(response.run_state.raw())),
        );
        invariant(invariant(global.rsp_features_mut()).set_value(response.features.raw()));
        invariant(invariant(global.rsp_epoch_mut()).set_value(response.epoch.raw()));
        invariant(
            invariant(global.max_ring_depth_mut())
                .set_value(u64::from(response.max_ring_depth.raw())),
        );
        invariant(
            invariant(global.max_indirect_destinations_mut())
                .set_value(u64::from(response.max_indirect_destinations.raw())),
        );
        invariant(
            invariant(global.max_transfer_bytes_mut())
                .set_value(u64::from(response.max_transfer_bytes.raw())),
        );
        invariant(
            invariant(global.max_strided_rows_mut())
                .set_value(u64::from(response.max_strided_rows.raw())),
        );
        invariant(
            invariant(global.max_stride_mut()).set_value(u64::from(response.max_stride.raw())),
        );
    });
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

fn low_u32(value: u64) -> u32 {
    u32::try_from(value).unwrap_or_default()
}

fn invariant<T, E>(result: Result<T, E>) -> T {
    let Ok(value) = result else {
        std::process::abort();
    };
    value
}
