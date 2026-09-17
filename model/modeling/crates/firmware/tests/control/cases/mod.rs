use pdma_model_software::ring::SoftRingError;
mod failures;
mod support;

use hardware_interface_runtime::SoftwareCpu;
use pdma_model_contract::{
    ControlQueue, ControlRegisters, ControlRequest, ControlRequestOpcode, ControlResponseStatus,
    ControlRingState,
};
use support::*;

const fn acquire(id: u64) -> ControlRequest {
    ControlRequest {
        id,
        opcode: ControlRequestOpcode::AcquireChannel,
        channel: 0,
    }
}

#[tokio::test]
async fn attach_drains_preexisting_command_and_notifies_both_progress_points() -> TestResult {
    // Given: Host publishes a command before firmware attaches.
    let (bus, memory) = fixture()?;
    publish(&memory, acquire(73), 1)?;
    let cpu = SoftwareCpu::new();
    let mut program = std::pin::pin!(cpu.run_main(pdma_model_firmware::main(&bus, memory.clone())));
    // When: firmware boots and drains the command.
    assert!(poll(program.as_mut()).is_pending());
    // Then: its response is published and command-space and response notifications both fire.
    assert_eq!(response(&memory)?.id, 73);
    assert_eq!(response(&memory)?.status, ControlResponseStatus::Success);
    assert_eq!(
        read_word(
            &memory,
            ControlQueue::COMMAND_OFFSET + ControlRingState::CONSUMER_OFFSET
        )?,
        1
    );
    assert_eq!(
        read_word(
            &memory,
            ControlQueue::RESPONSE_OFFSET + ControlRingState::PRODUCER_OFFSET
        )?,
        1
    );
    assert_eq!(
        bus.writes
            .lock()
            .unwrap()
            .iter()
            .filter(
                |(address, _)| *address == register(ControlRegisters::RESPONSE.offset()).unwrap()
            )
            .count(),
        2
    );
    Ok(())
}

#[tokio::test]
async fn idle_wait_requires_doorbell_and_retains_attached_geometry() -> TestResult {
    // Given: firmware attaches to empty rings and waits.
    let (bus, memory) = fixture()?;
    let cpu = SoftwareCpu::new();
    let mut program = std::pin::pin!(cpu.run_main(pdma_model_firmware::main(&bus, memory.clone())));
    assert!(poll(program.as_mut()).is_pending());
    publish(&memory, acquire(81), 1)?;
    write_word(
        &memory,
        ControlQueue::COMMAND_OFFSET + ControlRingState::CAPACITY_OFFSET,
        0,
    )?;
    assert!(poll(program.as_mut()).is_pending());
    assert_eq!(
        read_word(
            &memory,
            ControlQueue::COMMAND_OFFSET + ControlRingState::CONSUMER_OFFSET
        )?,
        0
    );
    // When: Host notifies firmware of the new request.
    bus.notification.notify_one();
    assert!(poll(program.as_mut()).is_pending());
    // Then: the request completes using geometry retained at attachment.
    assert_eq!(response(&memory)?.id, 81);
    Ok(())
}

#[tokio::test]
async fn full_response_ring_waits_for_host_consumption_notification() -> TestResult {
    // Given: one response fills the single-slot ring and a second request is queued.
    let (bus, memory) = fixture()?;
    publish(&memory, acquire(1), 1)?;
    let cpu = SoftwareCpu::new();
    let mut program = std::pin::pin!(cpu.run_main(pdma_model_firmware::main(&bus, memory.clone())));
    assert!(poll(program.as_mut()).is_pending());
    publish(&memory, acquire(2), 2)?;
    assert!(poll(program.as_mut()).is_pending());
    write_word(
        &memory,
        ControlQueue::RESPONSE_OFFSET + ControlRingState::CONSUMER_OFFSET,
        1,
    )?;
    assert!(poll(program.as_mut()).is_pending());
    assert_eq!(
        read_word(
            &memory,
            ControlQueue::COMMAND_OFFSET + ControlRingState::CONSUMER_OFFSET
        )?,
        1
    );
    // When: Host announces the freed response slot.
    bus.notification.notify_one();
    assert!(poll(program.as_mut()).is_pending());
    // Then: the next request executes with existing allocation state intact.
    assert_eq!(response(&memory)?.id, 2);
    assert_eq!(
        response(&memory)?.status,
        ControlResponseStatus::NoResources
    );
    Ok(())
}

#[tokio::test]
async fn malformed_descriptor_is_rejected_at_attachment() -> TestResult {
    // Given: Host enables an invalid descriptor.
    let (bus, memory) = fixture()?;
    write_word(
        &memory,
        ControlQueue::COMMAND_OFFSET + ControlRingState::CAPACITY_OFFSET,
        3,
    )?;
    // When: firmware attaches.
    let result = SoftwareCpu::new()
        .run_main(pdma_model_firmware::main(&bus, memory))
        .await;
    // Then: invalid geometry reaches the supervisor.
    assert!(matches!(result, Err(pdma_model_firmware::Error::Ring(_))));
    Ok(())
}

#[tokio::test]
async fn invalid_live_cursor_is_rejected_without_consuming_command() -> TestResult {
    // Given: firmware waits on an attached empty ring.
    let (bus, memory) = fixture()?;
    let cpu = SoftwareCpu::new();
    let mut program = std::pin::pin!(cpu.run_main(pdma_model_firmware::main(&bus, memory.clone())));
    assert!(poll(program.as_mut()).is_pending());
    write_word(
        &memory,
        ControlQueue::COMMAND_OFFSET + ControlRingState::PRODUCER_OFFSET,
        2,
    )?;
    // When: Host notifies with a cursor exceeding capacity.
    bus.notification.notify_one();
    let result = poll(program.as_mut());
    // Then: firmware rejects the cursor and leaves its consumer untouched.
    assert!(matches!(
        result,
        std::task::Poll::Ready(Err(pdma_model_firmware::Error::SoftRing(
            SoftRingError::Ring(_)
        )))
    ));
    assert_eq!(
        read_word(
            &memory,
            ControlQueue::COMMAND_OFFSET + ControlRingState::CONSUMER_OFFSET
        )?,
        0
    );
    Ok(())
}
