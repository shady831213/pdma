use super::TestResult;
use pdma_model_contract::semantic::{
    Command, CommandId, CompletionStatus, DmaRangeStart, QueueConfig, SoftwareOwnerId,
    desc::StridedCopyRequest, queue::EventRouting,
};
use pdma_model_device::SyncDevice;

pub(super) fn exercise() -> TestResult {
    let mut device = SyncDevice::from_core(pdma_model_device::DeviceCore::new(
        std::sync::Arc::new(hardware_interface_runtime::SharedMemory::new()),
        pdma_model_contract::semantic::ResourceConfig::default(),
        std::sync::Arc::new(hardware_interface_runtime::NoopRegisterObserver),
    ));
    let queue = device.create_queue(QueueConfig::new(
        2,
        2,
        EventRouting::QueueLocal,
        SoftwareOwnerId::new(1),
    ))?;
    let source = device.allocate_buffer(12)?;
    let destination = device.allocate_buffer(12)?;
    device.write_buffer(source, 0, &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11])?;
    device.write_buffer(destination, 0, &[80; 12])?;
    let command = Command::new(
        CommandId::new(0),
        pdma_model_contract::semantic::CommandOperation::StridedCopy(
            StridedCopyRequest::with_starts(
                DmaRangeStart::new(source, 1),
                DmaRangeStart::new(destination, 2),
                2,
                2,
                4,
                4,
            ),
        ),
    );

    device.submit_command(queue, command)?;
    device.ring_doorbell(queue)?;
    device.run_until_idle(2)?;

    let completion = device
        .poll_completion(queue)?
        .ok_or_else(|| std::io::Error::other("sync strided completion missing"))?;
    assert_eq!(completion.status(), CompletionStatus::Success);
    let destination_bytes = device.read_buffer(destination, 0, 12)?;
    assert_eq!(
        destination_bytes,
        vec![80, 80, 1, 2, 80, 80, 5, 6, 80, 80, 80, 80]
    );
    assert_eq!(device.poll_queue_event(queue)?, None);
    eprintln!("sync nonzero strided destination={destination_bytes:?}");
    Ok(())
}
