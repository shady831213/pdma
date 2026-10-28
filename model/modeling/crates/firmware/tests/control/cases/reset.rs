use super::{acquire, support::*};
use hw_model_io::memory::{MemoryAccess, MemoryAddress, MemoryAlignment};
use pdma_model_contract::control::{
    ControlQueue, ControlRequest, ControlRequestOpcode, ControlResponseStatus, ControlRingState,
};
use pdma_model_contract::dma::ChannelId;

#[tokio::test]
async fn reset_retains_allocation_and_reopens_only_after_hardware_completion() -> TestResult {
    // Given: the only channel is allocated and Host submits reset opcode two.
    let (bus, memory) = fixture()?;
    publish(&memory, acquire(1), 1)?;
    let mut program = std::pin::pin!(run(&bus, memory.clone()));
    assert!(poll(program.as_mut()).is_pending());
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
    memory.write(
        MemoryAddress::new(COMMAND + u64::try_from(ControlRequest::OPCODE_OFFSET)?),
        MemoryAlignment::ONE,
        &2_u32.to_le_bytes(),
    )?;
    bus.notification.raise();
    for _ in 0..4 {
        assert!(poll(program.as_mut()).is_pending());
    }
    let reset = u32::try_from(ChannelId::new(0).admin_offset(1).ok_or("admin offset")?)?;
    assert_eq!(
        bus.registers.lock().unwrap().get(&ADDRESSES.windows),
        Some(&0_u32.to_le_bytes().to_vec())
    );
    assert_eq!(
        bus.registers.lock().unwrap().get(&reset),
        Some(&1_u32.to_le_bytes().to_vec())
    );
    assert_eq!(read_word(&memory, producer)?, 1);
    // When: hardware clears its administration reset register.
    bus.registers
        .lock()
        .unwrap()
        .insert(reset, 0_u32.to_le_bytes().to_vec());
    for _ in 0..4 {
        assert!(poll(program.as_mut()).is_pending());
    }
    // Then: reset completes, the data aperture reopens, and acquisition cannot steal its allocation.
    assert_eq!(
        (response(&memory)?.id, response(&memory)?.status),
        (2, ControlResponseStatus::Success)
    );
    assert_eq!(
        bus.registers.lock().unwrap().get(&ADDRESSES.windows),
        Some(&1_u32.to_le_bytes().to_vec())
    );
    write_word(&memory, consumer, 2)?;
    publish(&memory, acquire(3), 3)?;
    bus.notification.raise();
    for _ in 0..4 {
        assert!(poll(program.as_mut()).is_pending());
    }
    assert_eq!(
        (response(&memory)?.id, response(&memory)?.status),
        (3, ControlResponseStatus::NoResources)
    );
    Ok(())
}
