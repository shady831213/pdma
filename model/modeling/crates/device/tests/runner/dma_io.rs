use pdma_model_contract::semantic::{
    CommandId, ExternalCommand,
    desc::{CopyCommand, CopyRequest},
    queue::{EventRouting, QueueConfig, SoftwareOwnerId},
};
use pdma_model_device::ModelError;
use pdma_model_device::SyncDevice;

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

pub(super) fn assert_dma_bus_decoded_command_uses_shared_core() -> TestResult {
    let mut device = SyncDevice::from_core(pdma_model_device::DeviceCore::new(
        std::sync::Arc::new(hardware_interface_runtime::SharedMemory::new()),
        pdma_model_contract::semantic::ResourceConfig::default(),
        std::sync::Arc::new(hardware_interface_runtime::NoopRegisterObserver),
    ));
    let queue = device.create_dma_bus_queue(QueueConfig::new(
        4,
        4,
        EventRouting::QueueLocal,
        SoftwareOwnerId::new(0),
    ))?;
    let source = device.allocate_buffer(4)?;
    let destination = device.allocate_buffer(4)?;
    let command = ExternalCommand::Copy(CopyCommand::new(
        CommandId::new(7),
        queue.queue_id(),
        queue.completion_ring_id(),
        CopyRequest::new(source, destination, 4),
    ));

    let result = device.submit_command(queue, command);

    assert_eq!(result, Err(ModelError::UnknownQueue(queue.queue_id())));
    assert_eq!(device.read_buffer(destination, 0, 4), Ok(vec![0; 4]));
    Ok(())
}
