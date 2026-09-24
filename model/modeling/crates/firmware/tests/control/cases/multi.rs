use super::{acquire, support::*};
use pdma_model_contract::control::{
    ControlQueue, ControlRequest, ControlRequestOpcode, ControlResponseStatus, ControlRingState,
};
use pdma_model_contract::dma::ChannelId;

#[tokio::test]
async fn duplicate_release_waits_then_reports_not_allocated() -> TestResult {
    // Given: the only allocation is being released while its hardware reset is busy.
    let (bus, memory) = fixture()?;
    publish(&memory, acquire(1), 1)?;
    let mut program = std::pin::pin!(run(&bus, memory.clone()));
    assert!(poll(program.as_mut()).is_pending());
    let consumer = ControlQueue::RESPONSE_OFFSET + ControlRingState::CONSUMER_OFFSET;
    let producer = ControlQueue::RESPONSE_OFFSET + ControlRingState::PRODUCER_OFFSET;
    write_word(&memory, consumer, 1)?;
    let release = ControlRequest {
        context: 0,
        attributes: pdma_model_contract::memory::ContextAttributes::DEFAULT,
        id: 2,
        function: 0,
        opcode: ControlRequestOpcode::ReleaseChannel,
        channel: 0,
    };
    publish(&memory, release, 2)?;
    bus.notification.raise();
    for _ in 0..4 {
        assert!(poll(program.as_mut()).is_pending());
    }
    // When: a duplicate release queues behind the pending lifecycle call.
    publish(&memory, ControlRequest { id: 3, ..release }, 3)?;
    bus.notification.raise();
    for _ in 0..4 {
        assert!(poll(program.as_mut()).is_pending());
    }
    // Then: it waits, observes the completed release, and cannot release the channel twice.
    assert_eq!(read_word(&memory, producer)?, 1);
    let reset = u32::try_from(ChannelId::new(0).admin_offset(1).ok_or("admin offset")?)?;
    bus.registers
        .lock()
        .unwrap()
        .insert(reset, 0_u32.to_le_bytes().to_vec());
    for _ in 0..4 {
        assert!(poll(program.as_mut()).is_pending());
    }
    assert_eq!(
        (response(&memory)?.id, response(&memory)?.status),
        (2, ControlResponseStatus::Success)
    );
    write_word(&memory, consumer, 2)?;
    bus.notification.raise();
    for _ in 0..4 {
        assert!(poll(program.as_mut()).is_pending());
    }
    assert_eq!(
        (response(&memory)?.id, response(&memory)?.status),
        (3, ControlResponseStatus::NotAllocated)
    );
    write_word(&memory, consumer, 3)?;
    publish(&memory, acquire(4), 4)?;
    bus.notification.raise();
    for _ in 0..4 {
        assert!(poll(program.as_mut()).is_pending());
    }
    assert_eq!(
        (
            response(&memory)?.id,
            response(&memory)?.status,
            response(&memory)?.channel
        ),
        (4, ControlResponseStatus::Success, 0)
    );
    Ok(())
}

#[tokio::test]
async fn management_waits_for_reset_before_consuming_the_next_request() -> TestResult {
    // Given: two firmware-managed channels, and a reset bus that stays busy until explicitly completed.
    let (bus, memory) = fixture()?;
    bus.registers
        .lock()
        .unwrap()
        .insert(0, 2_u32.to_le_bytes().to_vec());
    publish(&memory, acquire(1), 1)?;
    let mut program = std::pin::pin!(run(&bus, memory.clone()));
    assert!(poll(program.as_mut()).is_pending());
    assert_eq!(response(&memory)?.channel, 0);
    let consumer = ControlQueue::RESPONSE_OFFSET + ControlRingState::CONSUMER_OFFSET;
    let producer = ControlQueue::RESPONSE_OFFSET + ControlRingState::PRODUCER_OFFSET;
    write_word(&memory, consumer, 1)?;
    publish(
        &memory,
        ControlRequest {
            context: 0,
            attributes: pdma_model_contract::memory::ContextAttributes::DEFAULT,
            id: 2,
            function: 0,
            opcode: ControlRequestOpcode::ReleaseChannel,
            channel: 0,
        },
        2,
    )?;
    bus.notification.raise();
    for _ in 0..4 {
        assert!(poll(program.as_mut()).is_pending());
    }
    let reset = u32::try_from(ChannelId::new(0).admin_offset(2).unwrap())?;
    assert_eq!(
        bus.registers.lock().unwrap().get(&reset),
        Some(&1_u32.to_le_bytes().to_vec())
    );
    // When: Host requests another channel while the original release is still pending.
    publish(&memory, acquire(3), 3)?;
    bus.notification.raise();
    for _ in 0..8 {
        assert!(poll(program.as_mut()).is_pending());
    }
    // Then: neither a response nor consumption of the next request claims reset completion.
    assert_eq!(read_word(&memory, producer)?, 1);
    assert_eq!(
        read_word(
            &memory,
            ControlQueue::COMMAND_OFFSET + ControlRingState::CONSUMER_OFFSET
        )?,
        2
    );
    assert_eq!(
        bus.registers.lock().unwrap().get(&reset),
        Some(&1_u32.to_le_bytes().to_vec())
    );
    bus.registers
        .lock()
        .unwrap()
        .insert(reset, 0_u32.to_le_bytes().to_vec());
    bus.notification.raise();
    for _ in 0..4 {
        assert!(poll(program.as_mut()).is_pending());
    }
    assert_eq!(response(&memory)?.id, 2);
    write_word(&memory, consumer, 2)?;
    bus.notification.raise();
    for _ in 0..4 {
        assert!(poll(program.as_mut()).is_pending());
    }
    assert_eq!(response(&memory)?.id, 3);
    Ok(())
}
