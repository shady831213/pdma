use super::queue_control::{CommandResult, fail, map_abi};
use super::{QueueStaging, status_response};
use crate::runner::bus::control;
use crate::runner::state::RunnerState;
use pdma_model_contract::abi::mmio::MmioResult;
use pdma_model_contract::abi::{
    AbiControlError, DmaAddress, QueueActivation, QueueRingRegions, RingLayout, RingRecord,
};
use pdma_model_contract::{DeviceEpoch, EventRouting, QueueConfig, QueueId, SoftwareOwnerId};

pub(super) fn activate(state: &mut RunnerState, staging: QueueStaging) -> CommandResult {
    let queue_id = usize::try_from(staging.id)
        .map(QueueId::new)
        .map_err(|_| fail(MmioResult::InvalidTransaction, [0x1_0000_0001, 0, 0]))?;
    let owner = usize::try_from(staging.cfg_software_owner)
        .map(SoftwareOwnerId::new)
        .map_err(|_| fail(MmioResult::InvalidTransaction, [0x1_0000_0001, 0, 0]))?;
    let submission = layout(
        state,
        staging.sq_base,
        staging.sq_depth,
        RingRecord::Submission,
        0,
    )?;
    let completion = layout(
        state,
        staging.cq_base,
        staging.cq_depth,
        RingRecord::Completion,
        1,
    )?;
    let event = layout(
        state,
        staging.eq_base,
        staging.eq_depth,
        RingRecord::Event,
        2,
    )?;
    let config = QueueConfig::new(
        usize::try_from(staging.cfg_sq_depth)
            .map_err(|_| fail(MmioResult::InvalidTransaction, [0x1_0000_0001, 0, 0]))?,
        usize::try_from(staging.cfg_cq_depth)
            .map_err(|_| fail(MmioResult::InvalidTransaction, [0x1_0000_0001, 0, 0]))?,
        EventRouting::QueueLocal,
        owner,
    );
    let activation = QueueActivation::try_new(
        DeviceEpoch::new(staging.expected_epoch),
        queue_id,
        config,
        QueueRingRegions::new(submission, completion, event),
    )
    .map_err(|error| map_abi(error, activation_role(error)))?;
    let status = control::activation::activate_mapped(state, activation)
        .map_err(|error| super::queue_control::map_bus(error, staging.id, 1, 0))?;
    Ok((status_response(status)?, true))
}

fn layout(
    state: &RunnerState,
    base: u64,
    depth: u32,
    record: RingRecord,
    role: u64,
) -> Result<RingLayout, pdma_model_contract::abi::mmio::MmioResultDetails> {
    if depth == 0 {
        return Err(map_abi(
            AbiControlError::InvalidRingDepth {
                depth: 0,
                maximum: pdma_model_contract::abi::MAX_RING_DEPTH,
            },
            role,
        ));
    }
    if base % record.alignment() != 0 {
        return Err(map_abi(
            AbiControlError::MisalignedRing {
                base,
                alignment: record.alignment(),
            },
            role,
        ));
    }
    let extent = RingLayout::checked_extent(u64::from(depth), record.record_size())
        .map_err(|error| map_abi(error, role))?;
    let address = DmaAddress::new(base);
    let region = state
        .dma_space
        .region_containing(address)
        .ok_or_else(|| fail(MmioResult::DmaUnmapped, [base, extent, 0]))?;
    if region.base() != address {
        return Err(fail(MmioResult::DmaInvalidRegion, [base, extent, 0]));
    }
    RingLayout::try_new(region, u64::from(depth), record).map_err(|error| map_abi(error, role))
}

fn activation_role(error: pdma_model_contract::abi::AbiControlError) -> u64 {
    match error {
        pdma_model_contract::abi::AbiControlError::QueueDepthMismatch { ring, .. } => {
            u64::from(ring != "submission")
        }
        pdma_model_contract::abi::AbiControlError::EventDepthMismatch { .. } => 2,
        _ => 0,
    }
}
