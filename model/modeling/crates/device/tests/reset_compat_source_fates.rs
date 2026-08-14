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
    regions: &[pdma_model_contract::PdmaMemoryRegion],
    epoch: pdma_model_contract::DeviceEpoch,
) -> Result<pdma_model_contract::QueueActivation, pdma_model_contract::AbiControlError> {
    let [submission, completion, event] = regions else {
        return Err(pdma_model_contract::AbiControlError::QueueDepthMismatch {
            ring: "staged",
            expected: 3,
            actual: u32::MAX,
        });
    };
    pdma_model_contract::QueueActivation::try_new(
        epoch,
        pdma_model_contract::QueueId::new(0),
        pdma_model_contract::QueueConfig::new(
            2,
            2,
            pdma_model_contract::EventRouting::QueueLocal,
            pdma_model_contract::SoftwareOwnerId::new(0),
        ),
        pdma_model_contract::QueueRingRegions::new(
            pdma_model_contract::RingLayout::try_new(
                *submission,
                2,
                pdma_model_contract::RingRecord::Submission,
            )?,
            pdma_model_contract::RingLayout::try_new(
                *completion,
                2,
                pdma_model_contract::RingRecord::Completion,
            )?,
            pdma_model_contract::RingLayout::try_new(
                *event,
                2,
                pdma_model_contract::RingRecord::Event,
            )?,
        ),
    )
}
