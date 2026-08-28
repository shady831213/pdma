//! Executable Task 20 source-fate selector.

#[path = "reset_compat_source_fates/batch.rs"]
mod batch;
#[path = "reset_compat_source_fates/control.rs"]
mod control;
#[path = "reset_compat_source_fates/memory.rs"]
mod memory;
#[path = "reset_compat_source_fates/model.rs"]
mod model;

use std::error::Error;

type TestResult<T = ()> = Result<T, Box<dyn Error + Send + Sync>>;

fn require_error<T, E>(result: Result<T, E>) -> TestResult<E> {
    match result {
        Ok(_) => Err("expected a real error row".into()),
        Err(error) => Ok(error),
    }
}

fn activation(
    regions: &[pdma_model_device::MappedDmaRegion],
    epoch: pdma_model_contract::semantic::DeviceEpoch,
) -> Result<pdma_model_contract::semantic::QueueActivation, pdma_model_contract::abi::AbiControlError>
{
    let [submission, completion, event] = regions else {
        return Err(
            pdma_model_contract::abi::AbiControlError::QueueDepthMismatch {
                ring: "staged",
                expected: 3,
                actual: u32::MAX,
            },
        );
    };
    Ok(pdma_model_contract::semantic::QueueActivation::new(
        epoch,
        pdma_model_contract::semantic::QueueId::new(0),
        pdma_model_contract::semantic::QueueConfig::new(
            2,
            2,
            pdma_model_contract::semantic::EventRouting::QueueLocal,
            pdma_model_contract::semantic::SoftwareOwnerId::new(0),
        ),
        pdma_model_contract::semantic::QueueRingObjects::new(
            submission.object(),
            completion.object(),
            event.object(),
        ),
    ))
}
