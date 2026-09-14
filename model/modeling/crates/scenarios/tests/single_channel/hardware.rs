//! Hardware assembly: injected memory, device state and the existing stage graph.

use hardware_interface_runtime::MemoryAccess;
use pdma_model_device::{BuildError, Device, Pipeline};

/// Assemble the existing hardware state and stage graph over injected memory.
pub fn assemble<M: MemoryAccess>(memory: M) -> Result<(Device<M>, Pipeline), BuildError> {
    let device = Device::new(memory, 1)?;
    let pipeline = device.pipeline();
    Ok((device, pipeline))
}
