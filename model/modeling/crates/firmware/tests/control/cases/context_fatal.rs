use super::{acquire, contexts::table, support::*};
use pdma_model_contract::{
    control::{
        ControlQueue, ControlRequest, ControlRequestOpcode, ControlResponseStatus, ControlRingState,
    },
    dma::{DmaControl, DmaDiscovery},
    hw_components::context::{
        ContextEntryRegisters, ContextManagementRegisters, ContextTableRegisters,
    },
};

#[tokio::test]
async fn fatal_waits_for_accepted_context_maintenance_before_dma_recovery() -> TestResult {
    // Given: Destroy has requested hardware Drain, which is still pending.
    let (bus, memory) = fixture()?;
    table(&bus, 0)?;
    let base = 0x5000;
    let addresses = pdma_model_software::control::ManagementAddresses {
        contexts: Some(base),
        ..ADDRESSES
    };
    let result = base + u32::try_from(ContextManagementRegisters::RESULT.offset())?;
    bus.registers
        .lock()
        .unwrap()
        .insert(result, 1_u32.to_le_bytes().to_vec());
    publish(
        &memory,
        ControlRequest {
            opcode: ControlRequestOpcode::DestroyContext,
            ..acquire(1)
        },
        1,
    )?;
    let mut program = std::pin::pin!(run_at(&bus, memory.clone(), addresses));
    for _ in 0..4 {
        assert!(poll(program.as_mut()).is_pending());
    }
    let fault = u32::try_from(DmaDiscovery::EXTENT + DmaControl::FATAL.offset())?;
    // When: fatal interrupts maintenance before hardware has released its interlock.
    bus.registers
        .lock()
        .unwrap()
        .insert(fault, 1_u32.to_le_bytes().to_vec());
    bus.notification.raise();
    for _ in 0..8 {
        assert!(poll(program.as_mut()).is_pending());
    }
    // Then: no cleanup acknowledgment precedes the hardware terminal result.
    let producer = ControlQueue::RESPONSE_OFFSET + ControlRingState::PRODUCER_OFFSET;
    assert_eq!(read_word(&memory, producer)?, 0);
    bus.registers
        .lock()
        .unwrap()
        .insert(result, 3_u32.to_le_bytes().to_vec());
    for _ in 0..8 {
        assert!(poll(program.as_mut()).is_pending());
    }
    assert_eq!(read_word(&memory, producer)?, 1);
    assert_eq!(
        response(&memory)?.status,
        ControlResponseStatus::DeviceFailed
    );
    let valid = base
        + u32::try_from(
            ContextTableRegisters::for_channels(1, 1)?
                .entries()
                .offset(0)
                .ok_or("entry")?
                + ContextEntryRegisters::VALID.offset(),
        )?;
    assert_eq!(
        bus.registers.lock().unwrap().get(&valid),
        Some(&1_u32.to_le_bytes().to_vec())
    );
    write_word(
        &memory,
        ControlQueue::RESPONSE_OFFSET + ControlRingState::CONSUMER_OFFSET,
        1,
    )?;
    publish(
        &memory,
        ControlRequest {
            opcode: ControlRequestOpcode::ResetDma,
            ..acquire(2)
        },
        2,
    )?;
    bus.notification.raise();
    for _ in 0..8 {
        assert!(poll(program.as_mut()).is_pending());
    }
    let reset = u32::try_from(DmaDiscovery::EXTENT + DmaControl::RESET.offset())?;
    assert_eq!(
        bus.registers.lock().unwrap().get(&reset),
        Some(&1_u32.to_le_bytes().to_vec())
    );
    bus.registers
        .lock()
        .unwrap()
        .insert(fault, 0_u32.to_le_bytes().to_vec());
    bus.registers
        .lock()
        .unwrap()
        .insert(reset, 0_u32.to_le_bytes().to_vec());
    for _ in 0..8 {
        assert!(poll(program.as_mut()).is_pending());
    }
    assert_eq!(response(&memory)?.status, ControlResponseStatus::Success);
    assert_eq!(
        bus.registers.lock().unwrap().get(&valid),
        Some(&1_u32.to_le_bytes().to_vec())
    );
    Ok(())
}
