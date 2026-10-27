use super::{acquire, support::*};
use hardware_interface_io::memory::{MemoryAccess, MemoryAddress, MemoryAlignment};
use pdma_model_contract::hw_components::control::ControlNotificationRegisters;
use pdma_model_contract::{
    control::{ControlQueue, ControlResponseStatus, ControlRingState},
    dma::{DmaControl, DmaDiscovery},
    hw_components::control::ControlRegisters,
};

#[tokio::test]
async fn invalid_event_geometry_does_not_stop_command_service() -> TestResult {
    // Given: only the event descriptor is corrupt.
    let (bus, memory) = fixture()?;
    write_word(
        &memory,
        ControlQueue::EVENT_OFFSET + ControlRingState::CAPACITY_OFFSET,
        3,
    )?;
    publish(&memory, acquire(1), 1)?;
    let mut program = std::pin::pin!(run(&bus, memory.clone()));
    // When: firmware attaches and accepts an ordinary request.
    for _ in 0..8 {
        assert!(poll(program.as_mut()).is_pending());
    }
    // Then: invalid event geometry is reported independently of request success.
    assert_eq!(response(&memory)?.status, ControlResponseStatus::Success);
    assert_eq!(
        bus.registers.lock().unwrap().get(&register(
            ControlRegisters::NOTIFICATION.offset()
                + ControlNotificationRegisters::EVENT_ERROR.offset()
        )?),
        Some(&3_u32.to_le_bytes().to_vec())
    );
    Ok(())
}

#[tokio::test]
async fn event_full_access_failure_and_bad_cursors_stop_publication_without_retry() -> TestResult {
    for (mode, expected) in [(0, 1_u32), (1, 2), (2, 3)] {
        tokio::task::yield_now().await;
        // Given: an attached event sink with a full ring, an unmapped buffer, or bad cursors.
        let (bus, memory) = fixture()?;
        if mode == 1 {
            memory.write(
                MemoryAddress::new(QUEUE + u64::try_from(ControlQueue::EVENT_OFFSET)?),
                MemoryAlignment::ONE,
                &0xffff_0000_u64.to_le_bytes(),
            )?;
        }
        let mut program = std::pin::pin!(run(&bus, memory.clone()));
        for _ in 0..4 {
            assert!(poll(program.as_mut()).is_pending());
        }
        let producer = ControlQueue::EVENT_OFFSET + ControlRingState::PRODUCER_OFFSET;
        let initial = if mode == 0 {
            1
        } else if mode == 2 {
            2
        } else {
            0
        };
        write_word(&memory, producer, initial)?;
        memory.write(MemoryAddress::new(0x1300), MemoryAlignment::ONE, &[0xa5; 8])?;
        // When: fatal requires an event and firmware is repeatedly woken afterwards.
        bus.registers.lock().unwrap().insert(
            u32::try_from(DmaDiscovery::EXTENT + DmaControl::FATAL.offset())?,
            1_u32.to_le_bytes().to_vec(),
        );
        for _ in 0..8 {
            bus.notification.raise();
            assert!(poll(program.as_mut()).is_pending());
        }
        // Then: first error is retained, no slot is overwritten, and requests still complete.
        assert_eq!(
            bus.registers.lock().unwrap().get(&register(
                ControlRegisters::NOTIFICATION.offset()
                    + ControlNotificationRegisters::EVENT_ERROR.offset()
            )?),
            Some(&expected.to_le_bytes().to_vec())
        );
        assert_eq!(read_word(&memory, producer)?, initial);
        assert_eq!(
            memory.read_buffer::<[u8; 8]>(MemoryAddress::new(0x1300), MemoryAlignment::ONE)?,
            [0xa5; 8]
        );
        publish(&memory, acquire(1), 1)?;
        bus.notification.raise();
        for _ in 0..8 {
            assert!(poll(program.as_mut()).is_pending());
        }
        assert_eq!(
            read_word(
                &memory,
                ControlQueue::RESPONSE_OFFSET + ControlRingState::PRODUCER_OFFSET
            )?,
            1
        );
        assert_eq!(response(&memory)?.id, 1);
        assert_eq!(
            response(&memory)?.status,
            ControlResponseStatus::DeviceFailed
        );
        assert_eq!(
            bus.writes
                .lock()
                .unwrap()
                .iter()
                .filter(|(address, _)| *address
                    == register(
                        ControlRegisters::NOTIFICATION.offset()
                            + ControlNotificationRegisters::EVENT.offset()
                    )
                    .unwrap())
                .count(),
            1
        );
    }
    Ok(())
}
