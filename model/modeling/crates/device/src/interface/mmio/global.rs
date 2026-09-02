use crate::device::DeviceCore;
use pdma_model_contract::abi::mmio::{
    GlobalCommand, GlobalResponseBank, MmioResult, MmioResultDetails, ResponseUpdate,
};
use pdma_model_contract::abi::{AbiLimits, DiscoveryRequest};
use pdma_model_contract::semantic::DeviceRunState;

mod error;

pub(super) fn execute(state: &mut DeviceCore, raw: u32) -> bool {
    let Ok(command) = GlobalCommand::try_from(raw) else {
        complete_failure(
            state,
            details(MmioResult::UnknownCommand, [u64::from(raw), 0, 0]),
        );
        return false;
    };
    match command {
        GlobalCommand::Discover => {
            discover(state);
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
        GlobalCommand::Reset => reset(state),
    }
}

fn reset(state: &mut DeviceCore) -> bool {
    let expected_epoch = state.registers.global.reset_expected_epoch;
    match super::reset::execute(state, expected_epoch) {
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

fn discover(state: &mut DeviceCore) {
    let request = {
        let bank = &state.registers.global;
        DiscoveryRequest::try_new(low_u16(bank.req_abi_version), bank.req_features)
    };
    match request {
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

fn complete_success(state: &mut DeviceCore, response: GlobalResponseBank) {
    state.registers.global.command.complete(
        details(MmioResult::Ok, [0; 3]),
        ResponseUpdate::Replace(response),
    );
}

fn complete_failure(state: &mut DeviceCore, result: MmioResultDetails) {
    state
        .registers
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
