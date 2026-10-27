use super::{acquire, support::*};
use hardware_interface_io::memory::{MemoryAccess, MemoryAddress, MemoryAlignment};
use pdma_model_contract::control::{
    ControlEvent, ControlEventKind, ControlQueue, ControlRingState,
};
use pdma_model_contract::dma::{DmaControl, DmaDiscovery};
use pdma_model_contract::hw_components::control::ControlConfiguration;

#[tokio::test]
async fn startup_fatal_is_reported_once_after_host_attaches() -> TestResult {
    use pdma_model_contract::hw_components::control::ControlRegisters;
    // Given: hardware failed before PF enabled its control rings.
    let (bus, memory) = fixture()?;
    let enable =
        register(ControlRegisters::CONFIG.offset() + ControlConfiguration::ENABLE.offset())?;
    bus.registers
        .lock()
        .unwrap()
        .insert(enable, 0_u32.to_le_bytes().to_vec());
    bus.registers.lock().unwrap().insert(
        u32::try_from(DmaDiscovery::EXTENT + DmaControl::FATAL.offset())?,
        1_u32.to_le_bytes().to_vec(),
    );
    let mut program = std::pin::pin!(run(&bus, memory.clone()));
    for _ in 0..4 {
        assert!(poll(program.as_mut()).is_pending());
    }
    // When: PF later enables the initialized management descriptor.
    bus.registers
        .lock()
        .unwrap()
        .insert(enable, 1_u32.to_le_bytes().to_vec());
    bus.notification.raise();
    for _ in 0..8 {
        assert!(poll(program.as_mut()).is_pending());
    }
    // Then: the original episode is reported without requiring an IRQ replay.
    assert_eq!(
        read_word(
            &memory,
            ControlQueue::EVENT_OFFSET + ControlRingState::PRODUCER_OFFSET
        )?,
        1
    );
    bus.notification.raise();
    for _ in 0..4 {
        assert!(poll(program.as_mut()).is_pending());
    }
    assert_eq!(
        read_word(
            &memory,
            ControlQueue::EVENT_OFFSET + ControlRingState::PRODUCER_OFFSET
        )?,
        1
    );
    Ok(())
}

#[tokio::test]
async fn fatal_during_reset_finishes_without_reopening_or_reissuing_reset() -> TestResult {
    // Given: an allocated channel and a pending hardware reset.
    let (bus, memory) = fixture()?;
    publish(&memory, acquire(1), 1)?;
    let mut program = std::pin::pin!(run(&bus, memory.clone()));
    for _ in 0..4 {
        assert!(poll(program.as_mut()).is_pending());
    }
    write_word(
        &memory,
        ControlQueue::RESPONSE_OFFSET + ControlRingState::CONSUMER_OFFSET,
        1,
    )?;
    publish(
        &memory,
        pdma_model_contract::control::ControlRequest {
            opcode: pdma_model_contract::control::ControlRequestOpcode::ResetChannel,
            ..acquire(2)
        },
        2,
    )?;
    bus.notification.raise();
    for _ in 0..4 {
        assert!(poll(program.as_mut()).is_pending());
    }
    let reset = u32::try_from(
        pdma_model_contract::dma::ChannelId::new(0)
            .admin_offset(1)
            .ok_or("reset")?,
    )?;
    // When: fatal interrupts that same request, then hardware completes with no new IRQ.
    bus.registers.lock().unwrap().insert(
        u32::try_from(DmaDiscovery::EXTENT + DmaControl::FATAL.offset())?,
        1_u32.to_le_bytes().to_vec(),
    );
    bus.notification.raise();
    for _ in 0..4 {
        assert!(poll(program.as_mut()).is_pending());
    }
    bus.registers
        .lock()
        .unwrap()
        .insert(reset, 0_u32.to_le_bytes().to_vec());
    for _ in 0..8 {
        assert!(poll(program.as_mut()).is_pending());
    }
    // Then: the failed response retires the original request; the grant remains closed.
    assert_eq!(response(&memory)?.id, 2);
    assert_eq!(
        response(&memory)?.status,
        pdma_model_contract::control::ControlResponseStatus::DeviceFailed
    );
    assert_eq!(
        bus.registers.lock().unwrap().get(&ADDRESSES.windows),
        Some(&0_u32.to_le_bytes().to_vec())
    );
    assert_eq!(
        bus.writes
            .lock()
            .unwrap()
            .iter()
            .filter(|(address, _)| *address == reset)
            .count(),
        1
    );
    Ok(())
}

#[tokio::test]
async fn fatal_closes_window_and_publishes_event_with_full_response_ring() -> TestResult {
    // Given: acquisition has filled the management response ring.
    let (bus, memory) = fixture()?;
    publish(&memory, acquire(1), 1)?;
    let mut program = std::pin::pin!(run(&bus, memory.clone()));
    for _ in 0..4 {
        assert!(poll(program.as_mut()).is_pending());
    }
    publish(&memory, acquire(2), 2)?;
    // When: a hardware fatal wakes firmware while no response space exists.
    let fault = u32::try_from(DmaDiscovery::EXTENT + DmaControl::FATAL.offset())?;
    bus.registers
        .lock()
        .unwrap()
        .insert(fault, 1_u32.to_le_bytes().to_vec());
    bus.notification.raise();
    for _ in 0..8 {
        assert!(poll(program.as_mut()).is_pending());
    }
    // Then: data access closes and an independent event is published without admitting request 2.
    assert_eq!(
        bus.registers.lock().unwrap().get(&ADDRESSES.windows),
        Some(&0_u32.to_le_bytes().to_vec())
    );
    assert_eq!(
        read_word(
            &memory,
            ControlQueue::EVENT_OFFSET + ControlRingState::PRODUCER_OFFSET
        )?,
        1
    );
    let bytes = memory.read_buffer::<[u8; 8]>(MemoryAddress::new(0x1300), MemoryAlignment::ONE)?;
    assert_eq!(
        ControlEvent::from_bytes(&bytes)?,
        ControlEvent {
            kind: ControlEventKind::DmaFatal,
            code: 1
        }
    );
    assert_eq!(
        read_word(
            &memory,
            ControlQueue::COMMAND_OFFSET + ControlRingState::CONSUMER_OFFSET
        )?,
        1
    );
    Ok(())
}
