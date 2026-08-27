use std::error::Error;

use pdma_model_contract::semantic::{DeviceEpoch, WholeDeviceReset};
use pdma_model_device::test_support::sync_state_snapshot;
use pdma_model_device::{PdmaControlError, PdmaResetError};

use super::support::TestResult;

#[path = "overflow/fixture.rs"]
mod fixture;

/// Executes the real public overflow-reset proof.
///
/// # Panics
///
/// Panics when a required exact error/source or zero-mutation assertion regresses.
pub fn run() -> TestResult<(bool, usize)> {
    let mut fixture = fixture::populated()?;
    let before = sync_state_snapshot(&fixture.device)?;
    fixture.assert_preconditions(&before);
    let overflow = fixture
        .device
        .reset(WholeDeviceReset::new(DeviceEpoch::new(u64::MAX)));
    let overflow = fixture::require_error(overflow)?;
    let expected = PdmaResetError::Control(PdmaControlError::Abi(
        pdma_model_contract::abi::AbiControlError::DeviceEpochOverflow { current: u64::MAX },
    ));
    let epoch_overflow = overflow == expected;
    assert!(epoch_overflow);
    let control = overflow
        .source()
        .and_then(|source| source.downcast_ref::<PdmaControlError>());
    assert_eq!(
        control,
        Some(&PdmaControlError::Abi(
            pdma_model_contract::abi::AbiControlError::DeviceEpochOverflow { current: u64::MAX }
        ))
    );
    assert_eq!(
        control.and_then(|error| error.source().and_then(|source| source.downcast_ref())),
        Some(&pdma_model_contract::abi::AbiControlError::DeviceEpochOverflow { current: u64::MAX })
    );
    let after = sync_state_snapshot(&fixture.device)?;
    let mutations = fixture.assert_unchanged(&before, &after);
    assert_eq!(mutations, 0);
    Ok((epoch_overflow, mutations))
}
