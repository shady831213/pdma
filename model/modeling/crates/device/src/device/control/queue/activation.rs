use crate::device::DeviceCore;
use crate::memory::{MappedDmaRegion, PdmaMemoryError};
use crate::queue::exact::PdmaControlError;
use pdma_model_contract::abi::{
    AbiCommand, AbiCompletion, AbiEvent, AbiRingEntry, DmaAddress, QueueRingLayouts, RingLayout,
};
use pdma_model_contract::semantic::QueueId;

impl DeviceCore {
    pub(crate) fn activate_direct(&mut self, index: u32) -> Result<(), PdmaControlError> {
        let registers = self.queues.configuration(index)?;
        if self.queues.is_active(index) {
            return Err(PdmaControlError::InvalidTransaction);
        }
        let submission =
            mapped_ring::<AbiCommand>(self, registers.sq_base.raw(), registers.sq_depth.raw())?;
        let completion =
            mapped_ring::<AbiCompletion>(self, registers.cq_base.raw(), registers.cq_depth.raw())?;
        let event =
            mapped_ring::<AbiEvent>(self, registers.eq_base.raw(), registers.eq_depth.raw())?;
        let regions = [submission.0, completion.0, event.0];
        self.memory.validate_region_set(&regions)?;
        if regions_overlap(submission.0, completion.0)
            || regions_overlap(submission.0, event.0)
            || regions_overlap(completion.0, event.0)
        {
            return Err(PdmaControlError::InvalidTransaction);
        }
        let queue =
            QueueId::new(usize::try_from(index).map_err(|_| PdmaControlError::InvalidQueue)?);
        self.queues.install_exact(
            queue,
            QueueRingLayouts::new(submission.1, completion.1, event.1),
        )
    }
}

fn mapped_ring<R: AbiRingEntry>(
    runner: &DeviceCore,
    base: u64,
    depth: u32,
) -> Result<(MappedDmaRegion, RingLayout<R>), PdmaControlError> {
    let address = DmaAddress::new(base);
    let region = runner
        .memory
        .region_containing(address)
        .ok_or(PdmaControlError::Dma(PdmaMemoryError::InvalidRegion))?;
    if region.base() != address {
        return Err(PdmaControlError::Dma(PdmaMemoryError::InvalidRegion));
    }
    let layout = RingLayout::<R>::try_new(region.region(), u64::from(depth))?;
    Ok((region, layout))
}

const fn regions_overlap(left: MappedDmaRegion, right: MappedDmaRegion) -> bool {
    left.base().raw() < right.exclusive_end().raw()
        && right.base().raw() < left.exclusive_end().raw()
}
