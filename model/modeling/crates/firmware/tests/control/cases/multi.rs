use super::{acquire, support::*};
use pdma_model_contract::{
    ChannelBlock, ChannelId, ChannelRegisters, ControlQueue, ControlRequest, ControlRequestOpcode,
    ControlResponseStatus, ControlRingState,
};

#[tokio::test]
async fn releasing_channel_is_neither_free_nor_released_twice() -> TestResult {
    // Given: the only channel is allocated and its release is waiting for reset.
    let (bus, memory) = fixture()?;
    publish(&memory, acquire(1), 1)?;
    let mut program = std::pin::pin!(run(&bus, memory.clone()));
    assert!(poll(program.as_mut()).is_pending());
    let consumer = ControlQueue::RESPONSE_OFFSET + ControlRingState::CONSUMER_OFFSET;
    write_word(&memory, consumer, 1)?;
    let release = ControlRequest {
        id: 2,
        opcode: ControlRequestOpcode::ReleaseChannel,
        channel: 0,
    };
    publish(&memory, release, 2)?;
    bus.notification.raise();
    for _ in 0..4 {
        assert!(poll(program.as_mut()).is_pending());
    }
    // When: Host submits a duplicate release followed by another allocation request.
    for (request, status) in [
        (
            ControlRequest { id: 3, ..release },
            ControlResponseStatus::Busy,
        ),
        (acquire(4), ControlResponseStatus::NoResources),
    ] {
        publish(&memory, request, u32::try_from(request.id)?)?;
        bus.notification.raise();
        for _ in 0..4 {
            assert!(poll(program.as_mut()).is_pending());
        }
        // Then: each request is rejected independently and the original release is still retained.
        assert_eq!(
            (response(&memory)?.id, response(&memory)?.status),
            (request.id, status)
        );
        write_word(&memory, consumer, u32::try_from(request.id - 1)?)?;
    }
    let reset = u32::try_from(
        ChannelId::new(0).register_offset().unwrap()
            + ChannelBlock::CONTROL.offset()
            + ChannelRegisters::RESET.offset(),
    )?;
    bus.registers
        .lock()
        .unwrap()
        .insert(reset, 0_u32.to_le_bytes().to_vec());
    bus.notification.raise();
    for _ in 0..4 {
        assert!(poll(program.as_mut()).is_pending());
    }
    assert_eq!(
        (response(&memory)?.id, response(&memory)?.status),
        (2, ControlResponseStatus::Success)
    );
    write_word(&memory, consumer, 4)?;
    publish(&memory, acquire(5), 5)?;
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
        (5, ControlResponseStatus::Success, 0)
    );
    Ok(())
}

#[tokio::test]
async fn reset_of_one_channel_does_not_block_acquisition_of_another() -> TestResult {
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
            id: 2,
            opcode: ControlRequestOpcode::ReleaseChannel,
            channel: 0,
        },
        2,
    )?;
    bus.notification.raise();
    for _ in 0..4 {
        assert!(poll(program.as_mut()).is_pending());
    }
    let reset = u32::try_from(
        ChannelId::new(0).register_offset().unwrap()
            + ChannelBlock::CONTROL.offset()
            + ChannelRegisters::RESET.offset(),
    )?;
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
    // Then: the new acquisition completes first, without claiming the old reset completed.
    assert_eq!(read_word(&memory, producer)?, 2);
    let result = response(&memory)?;
    assert_eq!(
        (result.id, result.status, result.channel),
        (3, ControlResponseStatus::Success, 1)
    );
    assert_eq!(
        bus.registers.lock().unwrap().get(&reset),
        Some(&1_u32.to_le_bytes().to_vec())
    );
    write_word(&memory, consumer, 2)?;
    bus.registers
        .lock()
        .unwrap()
        .insert(reset, 0_u32.to_le_bytes().to_vec());
    bus.notification.raise();
    for _ in 0..4 {
        assert!(poll(program.as_mut()).is_pending());
    }
    assert_eq!(response(&memory)?.id, 2);
    Ok(())
}
