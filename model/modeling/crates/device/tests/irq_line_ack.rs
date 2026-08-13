#![allow(missing_docs)]

use pdma_model_device::ModelResult;

use pdma_model_contract::{
    CopyRequest, EventRouting, QueueConfig, SoftwareOwnerId,
    desc::{CopyCommand, ExternalCommand},
    ids::CommandId,
};
use pdma_model_device::SyncDevice;

#[test]
fn line_ack_preserves_a_cause_relatched_after_the_final_empty_poll() -> ModelResult<()> {
    // Given: software has drained one Line-routed completion and observed the ring empty.
    let mut device = SyncDevice::from_core(pdma_model_device::DeviceCore::new(
        std::sync::Arc::new(hardware_interface_runtime::SharedMemory::new()),
        pdma_model_contract::ResourceConfig::default(),
        std::sync::Arc::new(hardware_interface_runtime::NoopRegisterObserver),
    ));
    let queue = device
        .create_queue(QueueConfig::new(
            4,
            4,
            EventRouting::QueueLocal,
            SoftwareOwnerId::new(0),
        ))
        .expect("queue setup is valid");
    let source = queue.completion_irq_source();
    device
        .enable_irq_source(source)
        .expect("completion IRQ source exists");
    device
        .unmask_irq_source(source)
        .expect("completion IRQ source exists");
    let src = device.allocate_buffer(4).expect("allocation");
    let first_dst = device.allocate_buffer(4).expect("allocation");
    let second_dst = device.allocate_buffer(4).expect("allocation");
    device
        .write_buffer(src, 0, &[1, 2, 3, 4])
        .expect("source write is in bounds");
    submit_copy(&mut device, queue, src, first_dst, 0)?;
    device.run_until_idle(1).expect("first copy retires");
    assert!(device.wait_irq_notification().is_some());
    assert!(
        device
            .poll_completion(queue)
            .expect("completion ring exists")
            .is_some()
    );
    assert_eq!(
        device
            .poll_completion(queue)
            .expect("completion ring exists"),
        None
    );

    // When: the device publishes another completion before software sends its ack.
    submit_copy(&mut device, queue, src, second_dst, 1)?;
    device.run_until_idle(1).expect("second copy retires");
    device
        .ack_irq_source(source)
        .expect("completion IRQ source exists");

    // Then: ack preserves the relatched Line cause because its completion is still pending.
    let snapshot = device
        .irq_snapshot()
        .expect("the relatched completion keeps the Line pending");
    assert!(snapshot.asserted());
    assert_eq!(snapshot.cause().source(), source.source());
    Ok(())
}

#[test]
fn line_ack_cannot_create_a_cause_for_a_disabled_source() -> ModelResult<()> {
    // Given: a disabled source has a completion record but never latched an IRQ cause.
    let mut device = SyncDevice::from_core(pdma_model_device::DeviceCore::new(
        std::sync::Arc::new(hardware_interface_runtime::SharedMemory::new()),
        pdma_model_contract::ResourceConfig::default(),
        std::sync::Arc::new(hardware_interface_runtime::NoopRegisterObserver),
    ));
    let queue = device.create_queue(QueueConfig::new(
        2,
        2,
        EventRouting::QueueLocal,
        SoftwareOwnerId::new(0),
    ))?;
    let source = queue.completion_irq_source();
    let src = device.allocate_buffer(4).expect("allocation");
    let dst = device.allocate_buffer(4).expect("allocation");
    device.write_buffer(src, 0, &[1, 2, 3, 4])?;
    submit_copy(&mut device, queue, src, dst, 0)?;
    device.run_until_idle(1)?;
    assert_eq!(device.irq_snapshot(), None);

    // When: software acknowledges that inactive source before enabling and unmasking it.
    device.ack_irq_source(source)?;
    device.enable_irq_source(source)?;
    device.unmask_irq_source(source)?;

    // Then: ack has not synthesized an interrupt cause from the queued record alone.
    assert_eq!(device.irq_snapshot(), None);
    Ok(())
}

fn submit_copy(
    device: &mut SyncDevice,
    queue: pdma_model_contract::QueueConfigHandle,
    src: pdma_model_contract::BufferId,
    dst: pdma_model_contract::BufferId,
    command_id: usize,
) -> ModelResult<()> {
    device.submit_command(
        queue,
        ExternalCommand::Copy(CopyCommand::new(
            CommandId::new(command_id),
            queue.queue_id(),
            queue.completion_ring_id(),
            CopyRequest::new(src, dst, 4),
        )),
    )?;
    device.ring_doorbell(queue)
}
