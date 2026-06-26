#![allow(missing_docs)]

use pdma_modeling::desc::{CopyRequest, StridedCopyRequest};
use pdma_modeling::{
    Driver, EventRouting, ModelError, PdmaDevice, QueueConfig, QueueId, SoftwareOwnerId,
};

#[test]
fn creates_distinct_directory_entries_and_completion_bindings_for_two_queues() {
    // Given: two explicit queue configurations with different software owners.
    let mut device = PdmaDevice::new();
    let config_a = QueueConfig::new(2, 3, EventRouting::QueueLocal, SoftwareOwnerId::new(10));
    let config_b = QueueConfig::new(4, 5, EventRouting::QueueLocal, SoftwareOwnerId::new(20));

    // When: the device creates both queues through its directory-backed API.
    let handle_a = device
        .create_queue(config_a)
        .expect("first queue config is valid");
    let handle_b = device
        .create_queue(config_b)
        .expect("second queue config is valid");
    let entry_a = device
        .queue_directory_entry(handle_a.queue_id())
        .expect("first directory entry exists");
    let entry_b = device
        .queue_directory_entry(handle_b.queue_id())
        .expect("second directory entry exists");

    // Then: IDs and completion bindings come from stable directory entries.
    assert_eq!(handle_a.queue_id(), QueueId::new(0));
    assert_eq!(handle_b.queue_id(), QueueId::new(1));
    assert_ne!(handle_a.completion_ring_id(), handle_b.completion_ring_id());
    assert_eq!(entry_a, handle_a.directory_entry());
    assert_eq!(entry_b, handle_b.directory_entry());
    assert_eq!(entry_a.config(), config_a);
    assert_eq!(entry_b.config(), config_b);
}

#[test]
fn enforces_command_depth_from_explicit_queue_config() {
    // Given: a driver created from an explicit one-slot queue config.
    let mut device = PdmaDevice::new();
    let mut driver = Driver::setup_with_config(
        &mut device,
        QueueConfig::new(1, 2, EventRouting::QueueLocal, SoftwareOwnerId::new(7)),
    )
    .expect("queue config is valid");
    let src = driver.allocate_buffer(&mut device, 1);
    let dst = driver.allocate_buffer(&mut device, 1);

    // When: software submits one command and tries to submit another before reclaim.
    let command_id = driver
        .submit_copy(&mut device, CopyRequest::new(src, dst, 1))
        .expect("first command has ring space");
    let second_submit = driver.submit_copy(&mut device, CopyRequest::new(src, dst, 1));

    // Then: the explicit command depth is enforced and the handle records ownership.
    assert_eq!(command_id.raw(), 0);
    assert_eq!(second_submit, Err(ModelError::DriverQueueFull));
    assert_eq!(driver.queue_config_handle().command_depth(), 1);
    assert_eq!(driver.queue_config_handle().completion_depth(), 2);
    assert_eq!(
        driver.queue_config_handle().software_owner(),
        SoftwareOwnerId::new(7)
    );
}

#[test]
fn driver_uses_config_handle_for_queue_local_event_polling() {
    // Given: two drivers with explicit queue-local event routing.
    let mut device = PdmaDevice::new();
    let mut driver_a = Driver::setup_with_config(
        &mut device,
        QueueConfig::new(1, 4, EventRouting::QueueLocal, SoftwareOwnerId::new(1)),
    )
    .expect("first queue config is valid");
    let mut driver_b = Driver::setup_with_config(
        &mut device,
        QueueConfig::new(1, 4, EventRouting::QueueLocal, SoftwareOwnerId::new(2)),
    )
    .expect("second queue config is valid");
    let bad_src = driver_a.allocate_buffer(&mut device, 4);
    let bad_dst = driver_a.allocate_buffer(&mut device, 4);
    let good_src = driver_b.allocate_buffer(&mut device, 1);
    let good_dst = driver_b.allocate_buffer(&mut device, 1);
    driver_a
        .submit_strided_copy(
            &mut device,
            StridedCopyRequest::new(bad_src, bad_dst, 2, 4, 6, 4),
        )
        .expect("bad command has ring space");
    assert!(matches!(
        device.run_until_idle(1),
        Err(ModelError::OutOfBounds { .. })
    ));

    // When: the non-owning driver polls events before the owning driver.
    let wrong_driver_event = driver_b
        .poll_event(&mut device)
        .expect("event polling succeeds");
    let owner_event = driver_a
        .poll_event(&mut device)
        .expect("event polling succeeds")
        .expect("owning queue event remains available");

    // Then: queue-local routing preserves ownership and only the owning slot is reclaimed.
    assert_eq!(wrong_driver_event, None);
    assert_eq!(owner_event.queue_id(), driver_a.queue_id());
    assert!(
        driver_a
            .submit_copy(&mut device, CopyRequest::new(bad_src, bad_dst, 1))
            .is_ok()
    );
    assert!(
        driver_b
            .submit_copy(&mut device, CopyRequest::new(good_src, good_dst, 1))
            .is_ok()
    );
    assert_eq!(
        driver_b.submit_copy(&mut device, CopyRequest::new(good_src, good_dst, 1)),
        Err(ModelError::DriverQueueFull)
    );
}
