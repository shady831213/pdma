#![allow(missing_docs)]

use pdma_model_contract::{CompletionStatus, desc::CopyRequest};
use pdma_model_device::PdmaDevice;
use pdma_model_host::Driver;

#[test]
fn default_policy_steps_runnable_queues_in_submission_order() {
    // Given: two queues with one runnable copy each, submitted in queue order.
    let mut device = PdmaDevice::new();
    let mut driver_a = Driver::setup_on(&mut device, 2, 2).expect("queue setup is valid");
    let mut driver_b = Driver::setup_on(&mut device, 2, 2).expect("queue setup is valid");
    let src_a = driver_a.allocate_buffer_on(&mut device, 1);
    let dst_a = driver_a.allocate_buffer_on(&mut device, 1);
    let src_b = driver_b.allocate_buffer_on(&mut device, 1);
    let dst_b = driver_b.allocate_buffer_on(&mut device, 1);
    driver_a
        .write_buffer_on(&mut device, src_a, &[10])
        .expect("source A write is in bounds");
    driver_b
        .write_buffer_on(&mut device, src_b, &[20])
        .expect("source B write is in bounds");
    let command_a = driver_a
        .submit_copy_on(&mut device, CopyRequest::new(src_a, dst_a, 1))
        .expect("queue A submission has ring space");
    let command_b = driver_b
        .submit_copy_on(&mut device, CopyRequest::new(src_b, dst_b, 1))
        .expect("queue B submission has ring space");

    // When: the device executes one deterministic step.
    assert_eq!(device.step(), Ok(true));
    let first_a = driver_a
        .poll_completion_on(&mut device)
        .expect("queue A completion ring exists");
    let first_b = driver_b
        .poll_completion_on(&mut device)
        .expect("queue B completion ring exists");

    // Then: the default policy selects the oldest runnable queue first.
    let completion_a = first_a.expect("queue A completes first");
    assert_eq!(completion_a.command_id(), command_a);
    assert_eq!(completion_a.status(), CompletionStatus::Success);
    assert_eq!(first_b, None);
    assert_eq!(driver_a.read_buffer_on(&device, dst_a, 1), Ok(vec![10]));
    assert_eq!(driver_b.read_buffer_on(&device, dst_b, 1), Ok(vec![0]));

    // When: the device executes the next step.
    assert_eq!(device.step(), Ok(true));
    let completion_b = driver_b
        .poll_completion_on(&mut device)
        .expect("queue B completion ring exists")
        .expect("queue B completes second");

    // Then: the second runnable queue completes next.
    assert_eq!(completion_b.command_id(), command_b);
    assert_eq!(completion_b.status(), CompletionStatus::Success);
    assert_eq!(driver_b.read_buffer_on(&device, dst_b, 1), Ok(vec![20]));
}

#[test]
fn round_robin_does_not_repeat_queue_while_peer_is_runnable() {
    // Given: one peer queue and one queue with repeated work are both runnable.
    let mut device = PdmaDevice::new();
    let mut peer = Driver::setup_on(&mut device, 1, 2).expect("peer queue setup is valid");
    let mut repeated = Driver::setup_on(&mut device, 2, 3).expect("repeated queue setup is valid");
    let peer_src = peer.allocate_buffer_on(&mut device, 1);
    let peer_dst = peer.allocate_buffer_on(&mut device, 1);
    let repeated_src = repeated.allocate_buffer_on(&mut device, 2);
    let repeated_dst = repeated.allocate_buffer_on(&mut device, 2);
    peer.write_buffer_on(&mut device, peer_src, &[30])
        .expect("peer source write is in bounds");
    repeated
        .write_buffer_on(&mut device, repeated_src, &[40, 50])
        .expect("repeated source write is in bounds");
    let peer_command = peer
        .submit_copy_on(&mut device, CopyRequest::new(peer_src, peer_dst, 1))
        .expect("peer submission has ring space");
    let repeated_first = repeated
        .submit_copy_on(&mut device, CopyRequest::new(repeated_src, repeated_dst, 1))
        .expect("first repeated submission has ring space");
    let repeated_second = repeated
        .submit_copy_on(&mut device, CopyRequest::new(repeated_src, repeated_dst, 2))
        .expect("second repeated submission has ring space");

    // When: two deterministic steps execute.
    assert_eq!(device.step(), Ok(true));
    assert_eq!(device.step(), Ok(true));
    let peer_completion = peer
        .poll_completion_on(&mut device)
        .expect("peer completion ring exists")
        .expect("peer queue is not starved");
    let repeated_completion = repeated
        .poll_completion_on(&mut device)
        .expect("repeated completion ring exists")
        .expect("repeated queue first command completes");

    // Then: each runnable queue completed once before the repeated queue runs again.
    assert_eq!(peer_completion.command_id(), peer_command);
    assert_eq!(repeated_completion.command_id(), repeated_first);
    assert_eq!(peer.read_buffer_on(&device, peer_dst, 1), Ok(vec![30]));
    assert_eq!(
        repeated.read_buffer_on(&device, repeated_dst, 2),
        Ok(vec![40, 0])
    );

    // When: the device executes the third step.
    assert_eq!(device.step(), Ok(true));
    let second_completion = repeated
        .poll_completion_on(&mut device)
        .expect("repeated completion ring exists")
        .expect("repeated queue second command completes third");

    // Then: the repeated queue's second command completes after its peer made progress.
    assert_eq!(second_completion.command_id(), repeated_second);
    assert_eq!(
        repeated.read_buffer_on(&device, repeated_dst, 2),
        Ok(vec![40, 50])
    );
}
