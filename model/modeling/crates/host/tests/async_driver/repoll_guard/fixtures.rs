use super::BlockingPort;
use hardware_interface_runtime::{SharedMemory, SoftwareCpu};
use pdma_model_contract::semantic::ResourceConfig;
use pdma_model_device::{AsyncDevice, AsyncDeviceWorker};
use pdma_model_host::{AsyncDriver, CopyRequest, DriverResult, adapter::abi::PdmaHostMemory};
use std::sync::Arc;

pub(super) async fn setup_copy(
    cpu: &SoftwareCpu,
    port: &mut BlockingPort,
) -> DriverResult<(AsyncDriver, CopyRequest)> {
    cpu.run_main(async {
        let driver = AsyncDriver::setup_on(port, 1, 1).await?;
        let src = driver.allocate_buffer_on(port, 4).await?;
        let dst = driver.allocate_buffer_on(port, 4).await?;
        Ok((driver, CopyRequest::new(src, dst, 4)))
    })
    .await
}

pub(super) fn fresh_device() -> (AsyncDevice, AsyncDeviceWorker, PdmaHostMemory) {
    let memory = PdmaHostMemory::new(Arc::new(SharedMemory::new()));
    let (device, worker) = AsyncDevice::new(memory.memory_access(), ResourceConfig::default());
    (device, worker, memory)
}
