use super::{DeviceExecution, io};
use crate::dma_space::PdmaMemoryMappings;
use crate::runner::abi::bus::control::BusControlState;
use pdma_model_contract::semantic::QueueDirectoryEntry;

#[derive(Debug)]
pub(super) struct QueueContext {
    pub(super) entry: QueueDirectoryEntry,
    pub(super) frontend: io::QueueFrontend,
}

pub(in crate::runner) struct NativeExecution<'a> {
    pub(in crate::runner) device: &'a mut DeviceExecution,
    pub(in crate::runner) dma_space: &'a mut PdmaMemoryMappings,
    pub(in crate::runner) bus_control: &'a mut BusControlState,
}
