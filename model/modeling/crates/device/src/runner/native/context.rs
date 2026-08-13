use super::{QueuePortDeviceModel, io};
use crate::dma_space::PdmaMemoryMappings;
use crate::runner::bus::control::BusControlState;
use pdma_model_contract::QueueDirectoryEntry;

#[derive(Debug)]
pub(super) struct QueueContext {
    pub(super) entry: QueueDirectoryEntry,
    pub(super) frontend: Box<dyn io::QueueFrontend>,
}

pub(in crate::runner) struct NativeExecution<'a> {
    pub(in crate::runner) device: &'a mut QueuePortDeviceModel,
    pub(in crate::runner) dma_space: &'a mut PdmaMemoryMappings,
    pub(in crate::runner) bus_control: &'a mut BusControlState,
}
