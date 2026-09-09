use crate::device::DeviceCore;
use crate::queue::exact::PdmaControlError;
use hardware_interface_runtime::{MemoryAccess, MemoryAccessError, MemoryAddress, MemoryAlignment};
use pdma_model_contract::abi::{
    AbiCommand, AbiCompletion, AbiEvent, AbiRingEntry, DmaAddress, DmaRegion, QueueRingLayouts,
    RingLayout,
};
use pdma_model_contract::semantic::QueueId;

impl DeviceCore {
    pub(crate) fn activate_direct(&mut self, index: u32) -> Result<(), PdmaControlError> {
        let registers = self.queues.configuration(index)?;
        if self.queues.is_active(index) {
            return Err(PdmaControlError::InvalidTransaction);
        }
        let submission =
            ring_layout::<AbiCommand>(self, registers.sq_base.raw(), registers.sq_depth.raw())?;
        let completion =
            ring_layout::<AbiCompletion>(self, registers.cq_base.raw(), registers.cq_depth.raw())?;
        let event =
            ring_layout::<AbiEvent>(self, registers.eq_base.raw(), registers.eq_depth.raw())?;
        if regions_overlap(submission.region(), completion.region())
            || regions_overlap(submission.region(), event.region())
            || regions_overlap(completion.region(), event.region())
        {
            return Err(PdmaControlError::InvalidTransaction);
        }
        let queue =
            QueueId::new(usize::try_from(index).map_err(|_| PdmaControlError::InvalidQueue)?);
        self.queues
            .install_exact(queue, QueueRingLayouts::new(submission, completion, event))
    }
}

fn ring_layout<R: AbiRingEntry>(
    runner: &DeviceCore,
    base: u64,
    depth: u32,
) -> Result<RingLayout<R>, PdmaControlError> {
    let address = DmaAddress::new(base);
    let length = RingLayout::<R>::checked_extent(u64::from(depth))?;
    let region =
        DmaRegion::try_new(address, length).map_err(|_| MemoryAccessError::AddressOverflow {
            address: MemoryAddress::new(base),
            length,
        })?;
    let layout = RingLayout::<R>::try_new(region, u64::from(depth))?;
    runner
        .memory
        .validate(MemoryAddress::new(base), MemoryAlignment::ONE, length)?;
    Ok(layout)
}

const fn regions_overlap(left: DmaRegion, right: DmaRegion) -> bool {
    left.base().raw() < right.exclusive_end().raw()
        && right.base().raw() < left.exclusive_end().raw()
}
