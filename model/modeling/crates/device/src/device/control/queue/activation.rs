//! Exact queue activation preparation and commit.

use crate::device::DeviceCore;
use crate::memory::{MappedDmaRegion, PdmaMemoryError};
use crate::queue::exact::{PdmaControlError, PreparedExactQueue, QueueStatus};
use pdma_model_contract::abi::{
    AbiCommand, AbiCompletion, AbiEvent, AbiRingEntry, QueueRingLayouts, RingLayout,
};
use pdma_model_contract::semantic::QueueActivation;

struct ValidatedQueueActivation {
    request: QueueActivation,
    layouts: QueueRingLayouts,
}

struct PreparedQueueActivation {
    native: PreparedExactQueue,
    status: QueueStatus,
}

impl DeviceCore {
    pub(crate) fn activate_mapped_exact_queue(
        &mut self,
        activation: QueueActivation,
    ) -> Result<QueueStatus, PdmaControlError> {
        let prepared = self.prepare_mapped_exact_queue(activation)?;
        Ok(self.install_exact_queue_activation(prepared))
    }

    fn prepare_mapped_exact_queue(
        &mut self,
        activation: QueueActivation,
    ) -> Result<PreparedQueueActivation, PdmaControlError> {
        self.validate_exact_activation(activation)?;
        let regions = mapped_regions(self, activation)?;
        self.memory.validate_region_set(&regions)?;
        let validated = validate_layouts(activation, regions)?;
        self.prepare_exact_queue_activation(&validated)
    }

    fn validate_exact_activation(
        &self,
        activation: QueueActivation,
    ) -> Result<(), PdmaControlError> {
        if activation.queue().epoch() != self.epoch() {
            return Err(PdmaControlError::StaleEpoch {
                expected: self.epoch(),
                actual: activation.queue().epoch(),
            });
        }
        if activation.queue().queue_id() != self.next_queue_id() {
            return Err(PdmaControlError::InvalidQueue);
        }
        Ok(())
    }

    fn prepare_exact_queue_activation(
        &mut self,
        validated: &ValidatedQueueActivation,
    ) -> Result<PreparedQueueActivation, PdmaControlError> {
        let native = self
            .prepare_dma_bus_queue(PreparedExactQueue::new(
                validated.request,
                validated.layouts,
            ))
            .map_err(|_| PdmaControlError::InvalidTransaction)?;
        if native.queue_id() != validated.request.queue().queue_id() {
            return Err(PdmaControlError::InvalidTransaction);
        }
        let status = native.status()?;
        Ok(PreparedQueueActivation { native, status })
    }

    fn install_exact_queue_activation(&mut self, prepared: PreparedQueueActivation) -> QueueStatus {
        self.install_dma_bus_queue(prepared.native);
        prepared.status
    }
}

fn mapped_regions(
    runner: &DeviceCore,
    activation: QueueActivation,
) -> Result<[MappedDmaRegion; 3], PdmaControlError> {
    let objects = activation.rings();
    let resolve = |object| {
        runner
            .memory
            .region_for_object(object)
            .ok_or(PdmaControlError::Dma(PdmaMemoryError::InvalidRegion))
    };
    Ok([
        resolve(objects.submission())?,
        resolve(objects.completion())?,
        resolve(objects.event())?,
    ])
}

fn validate_layouts(
    activation: QueueActivation,
    regions: [MappedDmaRegion; 3],
) -> Result<ValidatedQueueActivation, PdmaControlError> {
    let config = activation.config();
    if u64::try_from(config.software_owner().raw()).is_err() {
        return Err(PdmaControlError::InvalidTransaction);
    }
    let [submission, completion, event] = regions;
    if regions_overlap(submission, completion)
        || regions_overlap(submission, event)
        || regions_overlap(completion, event)
    {
        return Err(PdmaControlError::InvalidTransaction);
    }
    let layouts = QueueRingLayouts::new(
        layout::<AbiCommand>(submission, config.command_depth())?,
        layout::<AbiCompletion>(completion, config.completion_depth())?,
        layout::<AbiEvent>(event, config.event_depth())?,
    );
    Ok(ValidatedQueueActivation {
        request: activation,
        layouts,
    })
}

fn layout<R: AbiRingEntry>(
    region: MappedDmaRegion,
    depth: usize,
) -> Result<RingLayout<R>, PdmaControlError> {
    let depth = u64::try_from(depth).map_err(|_| PdmaControlError::InvalidTransaction)?;
    RingLayout::try_new(region.region(), depth).map_err(PdmaControlError::from)
}

const fn regions_overlap(left: MappedDmaRegion, right: MappedDmaRegion) -> bool {
    left.base().raw() < right.exclusive_end().raw()
        && right.base().raw() < left.exclusive_end().raw()
}
