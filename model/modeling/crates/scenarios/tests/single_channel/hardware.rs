//! Hardware assembly: injected memory, device state and the existing stage graph.

use hardware_interface_runtime::MemoryAccess;
use pdma_model_device::{Device, Pipeline, pipeline};

/// Assemble the existing hardware state and stage graph over injected memory.
pub fn assemble<M: MemoryAccess>(memory: M) -> (Device<M>, Pipeline) {
    (Device::new(memory), pipeline())
}
