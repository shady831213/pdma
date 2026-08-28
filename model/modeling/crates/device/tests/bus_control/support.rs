use pdma_model_contract::semantic::{
    EventRouting, QueueActivation, QueueConfig, QueueId, QueueRingObjects, SoftwareOwnerId,
};
use pdma_model_device::AsyncDeviceHandle;

/// Creates a fresh asynchronous queue-port handle.
pub fn new_handle() -> AsyncDeviceHandle {
    let (device, _worker) =
        pdma_model_device::AsyncDevice::from_core(pdma_model_device::DeviceCore::new(
            std::sync::Arc::new(hardware_interface_runtime::SharedMemory::new()),
            pdma_model_contract::semantic::ResourceConfig::default(),
            std::sync::Arc::new(hardware_interface_runtime::NoopRegisterObserver),
        ));
    device.handle()
}

/// Builds the canonical depth-two activation for a staged ring batch.
pub fn activation(
    stage: &pdma_model_device::PdmaMemoryStage,
    queue_id: QueueId,
) -> Result<QueueActivation, pdma_model_contract::abi::AbiControlError> {
    let [submission_region, completion_region, event_region] = stage.regions() else {
        let actual = u32::try_from(stage.regions().len()).map_or(u32::MAX, |value| value);
        return Err(
            pdma_model_contract::abi::AbiControlError::QueueDepthMismatch {
                ring: "staged",
                expected: 3,
                actual,
            },
        );
    };
    Ok(QueueActivation::new(
        pdma_model_contract::semantic::DeviceEpoch::initial(),
        queue_id,
        QueueConfig::new(2, 2, EventRouting::QueueLocal, SoftwareOwnerId::new(7)),
        QueueRingObjects::new(
            submission_region.object(),
            completion_region.object(),
            event_region.object(),
        ),
    ))
}
