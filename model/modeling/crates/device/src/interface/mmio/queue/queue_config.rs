use super::queue_control::{CommandResult, fail, map_abi};
use super::{QueueRequest, status_response};
use crate::device::DeviceCore;
use pdma_model_contract::abi::mmio::MmioResult;
use pdma_model_contract::abi::{
    AbiCommand, AbiCompletion, AbiControlError, AbiEvent, AbiRingEntry, DmaAddress, RingLayout,
};
use pdma_model_contract::semantic::{
    BufferId, DeviceEpoch, EventRouting, QueueActivation, QueueConfig, QueueId, QueueRingObjects,
    SoftwareOwnerId,
};

#[derive(Clone, Copy)]
struct StagedRing {
    base: u64,
    depth: u32,
    kind: RingKind,
    role: u64,
}

#[derive(Clone, Copy)]
enum RingKind {
    Submission,
    Completion,
    Event,
}

pub(super) fn activate(state: &mut DeviceCore, staging: QueueRequest) -> CommandResult {
    let queue_id = usize::try_from(staging.id)
        .map(QueueId::new)
        .map_err(|_| fail(MmioResult::InvalidTransaction, [0x1_0000_0001, 0, 0]))?;
    let owner = usize::try_from(staging.cfg_software_owner)
        .map(SoftwareOwnerId::new)
        .map_err(|_| fail(MmioResult::InvalidTransaction, [0x1_0000_0001, 0, 0]))?;
    let rings = [
        StagedRing {
            base: staging.sq_base,
            depth: staging.sq_depth,
            kind: RingKind::Submission,
            role: 0,
        },
        StagedRing {
            base: staging.cq_base,
            depth: staging.cq_depth,
            kind: RingKind::Completion,
            role: 1,
        },
        StagedRing {
            base: staging.eq_base,
            depth: staging.eq_depth,
            kind: RingKind::Event,
            role: 2,
        },
    ];
    let [submission_ring, completion_ring, event_ring] = rings;
    let submission = ring_object::<AbiCommand>(state, submission_ring)?;
    let completion = ring_object::<AbiCompletion>(state, completion_ring)?;
    let event = ring_object::<AbiEvent>(state, event_ring)?;
    let config = QueueConfig::new(
        usize::try_from(staging.cfg_sq_depth)
            .map_err(|_| fail(MmioResult::InvalidTransaction, [0x1_0000_0001, 0, 0]))?,
        usize::try_from(staging.cfg_cq_depth)
            .map_err(|_| fail(MmioResult::InvalidTransaction, [0x1_0000_0001, 0, 0]))?,
        EventRouting::QueueLocal,
        owner,
    );
    validate_config_depths(config, rings)?;
    let activation = QueueActivation::new(
        DeviceEpoch::new(staging.expected_epoch),
        queue_id,
        config,
        QueueRingObjects::new(submission, completion, event),
    );
    let status = state
        .activate_mapped_exact_queue(activation)
        .map_err(|error| super::queue_control::map_bus(error, staging.id, 1, 0))?;
    Ok((status_response(status)?, true))
}

fn ring_object<R: AbiRingEntry>(
    state: &DeviceCore,
    staging: StagedRing,
) -> Result<BufferId, pdma_model_contract::abi::mmio::MmioResultDetails> {
    if staging.depth == 0 {
        return Err(map_abi(
            AbiControlError::InvalidRingDepth {
                depth: 0,
                maximum: pdma_model_contract::abi::MAX_RING_DEPTH,
            },
            staging.role,
        ));
    }
    if staging.base % R::RING_ALIGNMENT != 0 {
        return Err(map_abi(
            AbiControlError::MisalignedRing {
                base: staging.base,
                alignment: R::RING_ALIGNMENT,
            },
            staging.role,
        ));
    }
    let extent = RingLayout::<R>::checked_extent(u64::from(staging.depth))
        .map_err(|error| map_abi(error, staging.role))?;
    let address = DmaAddress::new(staging.base);
    let region = state
        .dma
        .region_containing(address)
        .ok_or_else(|| fail(MmioResult::DmaUnmapped, [staging.base, extent, 0]))?;
    if region.base() != address {
        return Err(fail(
            MmioResult::DmaInvalidRegion,
            [staging.base, extent, 0],
        ));
    }
    RingLayout::<R>::try_new(region.region(), u64::from(staging.depth))
        .map_err(|error| map_abi(error, staging.role))?;
    Ok(region.object())
}

fn validate_config_depths(
    config: QueueConfig,
    rings: [StagedRing; 3],
) -> Result<(), pdma_model_contract::abi::mmio::MmioResultDetails> {
    let expected = [
        config.command_depth(),
        config.completion_depth(),
        config.event_depth(),
    ];
    for (ring, expected) in rings.into_iter().zip(expected) {
        if usize::try_from(ring.depth) != Ok(expected) {
            let error = match ring.kind {
                RingKind::Event => AbiControlError::EventDepthMismatch {
                    expected,
                    actual: ring.depth,
                },
                RingKind::Submission => AbiControlError::QueueDepthMismatch {
                    ring: "submission",
                    expected,
                    actual: ring.depth,
                },
                RingKind::Completion => AbiControlError::QueueDepthMismatch {
                    ring: "completion",
                    expected,
                    actual: ring.depth,
                },
            };
            return Err(map_abi(error, ring.role));
        }
    }
    Ok(())
}
