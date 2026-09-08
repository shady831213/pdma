use super::DeviceCore;
use pdma_model_contract::abi::mmio::{
    GlobalCommand, GlobalResponseBank, MmioResult, MmioResultDetails,
};
use pdma_model_contract::abi::{AbiLimits, DiscoveryRequest};
use pdma_model_contract::semantic::{DeviceEpoch, DeviceRunState, WholeDeviceReset};

mod error;
#[cfg(test)]
mod tests;

impl DeviceCore {
    pub(super) fn execute_global_command(&mut self) -> bool {
        let raw = self.global_registers.command.raw();
        let outcome = GlobalCommand::try_from(raw)
            .map_err(|_| details(MmioResult::UnknownCommand, [u64::from(raw), 0, 0]))
            .and_then(|command| match command {
                GlobalCommand::Discover => {
                    let version = u16::try_from(self.global_registers.req_abi_version.raw())
                        .unwrap_or_default();
                    DiscoveryRequest::try_new(version, self.global_registers.req_features.raw())
                        .map_err(error::map_discovery)?;
                    let discovery = pdma_model_contract::abi::AbiDiscovery::v1();
                    Ok((
                        GlobalResponseBank {
                            abi_version: u32::from(discovery.version()).into(),
                            features: discovery.features().bits().into(),
                            ..GlobalResponseBank::default()
                        },
                        false,
                    ))
                }
                GlobalCommand::ReadLimits => {
                    let limits = AbiLimits::v1();
                    Ok((
                        GlobalResponseBank {
                            max_ring_depth: limits.max_ring_depth().into(),
                            max_indirect_destinations: limits.max_indirect_destinations().into(),
                            max_transfer_bytes: limits.max_transfer_bytes().into(),
                            max_strided_rows: limits.max_strided_rows().into(),
                            max_stride: limits.max_stride().into(),
                            ..GlobalResponseBank::default()
                        },
                        false,
                    ))
                }
                GlobalCommand::ReadStatus => {
                    let run_state = match self.run_state() {
                        DeviceRunState::Idle => 0,
                        DeviceRunState::Running => 1,
                        DeviceRunState::Blocked => 2,
                    };
                    Ok((
                        GlobalResponseBank {
                            run_state: run_state.into(),
                            epoch: self.epoch().raw().into(),
                            ..GlobalResponseBank::default()
                        },
                        false,
                    ))
                }
                GlobalCommand::Reset => {
                    let expected =
                        DeviceEpoch::new(self.global_registers.reset_expected_epoch.raw());
                    let epoch = self
                        .reset_runner(WholeDeviceReset::new(expected))
                        .map_err(error::map_reset)?;
                    Ok((
                        GlobalResponseBank {
                            epoch: epoch.raw().into(),
                            ..GlobalResponseBank::default()
                        },
                        true,
                    ))
                }
            });
        let (result, changed) = match outcome {
            Ok((response, changed)) => {
                self.global_registers.response = response;
                (details(MmioResult::Ok, [0; 3]), changed)
            }
            Err(error) => (error, false),
        };
        let registers = &mut self.global_registers;
        registers.result = result.result.code().into();
        let [detail0, detail1, detail2] = result.details;
        registers.detail0 = detail0.into();
        registers.detail1 = detail1.into();
        registers.detail2 = detail2.into();
        changed
    }
}

const fn details(result: MmioResult, details: [u64; 3]) -> MmioResultDetails {
    MmioResultDetails { result, details }
}
