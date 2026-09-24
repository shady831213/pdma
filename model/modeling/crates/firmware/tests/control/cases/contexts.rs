use super::{acquire, support::*};
use pdma_model_contract::control::{
    ControlQueue, ControlRequest, ControlRequestOpcode, ControlResponseStatus, ControlRingState,
};
use pdma_model_contract::dma::ChannelId;
use pdma_model_contract::hw_components::context::{
    ContextEntryRegisters, ContextManagementRegisters, ContextResultState, ContextResultValues,
    ContextTableRegisters,
};

const BASE: u32 = 0x5000;

#[tokio::test]
async fn destroy_waits_for_drain_then_firmware_clears_valid() -> TestResult {
    // Given: a valid binding and a hardware Drain result that remains Pending.
    let (bus, memory) = fixture()?;
    table(&bus, 0)?;
    let result = BASE + u32::try_from(ContextManagementRegisters::RESULT.offset())?;
    let valid = BASE
        + u32::try_from(
            ContextTableRegisters::for_channels(1, 1)?
                .entries()
                .offset(0)
                .ok_or("entry")?
                + ContextEntryRegisters::VALID.offset(),
        )?;
    bus.registers
        .lock()
        .unwrap()
        .insert(result, 1_u32.to_le_bytes().to_vec());
    publish(
        &memory,
        ControlRequest {
            opcode: ControlRequestOpcode::DestroyContext,
            ..acquire(8)
        },
        1,
    )?;
    let addresses = pdma_model_software::control::ManagementAddresses {
        contexts: Some(BASE),
        ..ADDRESSES
    };
    let mut program = std::pin::pin!(run_at(&bus, memory.clone(), addresses));
    // When: firmware submits Drain and waits across multiple polls.
    for _ in 0..4 {
        assert!(poll(program.as_mut()).is_pending());
    }
    assert_eq!(
        bus.registers.lock().unwrap().get(&valid),
        Some(&1_u32.to_le_bytes().to_vec())
    );
    assert_eq!(
        read_word(
            &memory,
            ControlQueue::RESPONSE_OFFSET + ControlRingState::PRODUCER_OFFSET
        )?,
        0
    );
    assert_eq!(
        bus.registers
            .lock()
            .unwrap()
            .get(&(BASE + u32::try_from(ContextManagementRegisters::REQUEST.offset())?)),
        Some(&(1_u64 << 32).to_le_bytes().to_vec())
    );
    bus.registers
        .lock()
        .unwrap()
        .insert(result, 2_u32.to_le_bytes().to_vec());
    for _ in 0..4 {
        assert!(poll(program.as_mut()).is_pending());
    }
    // Then: firmware writes valid zero before returning successful destruction.
    assert_eq!(
        bus.registers.lock().unwrap().get(&valid),
        Some(&0_u32.to_le_bytes().to_vec())
    );
    assert_eq!(
        (response(&memory)?.id, response(&memory)?.status),
        (8, ControlResponseStatus::Success)
    );
    Ok(())
}

fn table(bus: &Bus, owner: u32) -> TestResult {
    let entry = ContextTableRegisters::for_channels(1, 1)?
        .entries()
        .offset(0)
        .ok_or("entry")?;
    for (offset, value) in [
        (ContextManagementRegisters::CAPACITY.offset(), 1),
        (entry + ContextEntryRegisters::VALID.offset(), 1),
        (entry + ContextEntryRegisters::OWNER.offset(), owner),
        (
            ContextManagementRegisters::RESULT.offset(),
            ContextResultValues::default()
                .with_state(ContextResultState::Failed)
                .raw(),
        ),
        (
            ContextTableRegisters::for_channels(1, 1)?
                .failed()
                .offset(0)
                .ok_or("bitmap offset")?,
            1,
        ),
    ] {
        bus.registers
            .lock()
            .map_err(|_| "context register lock poisoned")?
            .insert(BASE + u32::try_from(offset)?, value.to_le_bytes().to_vec());
    }
    Ok(())
}

#[tokio::test]
async fn maintenance_failure_returns_then_reset_is_serviced() -> TestResult {
    // Given: an allocated channel and a context whose hardware cleanup fails.
    let (bus, memory) = fixture()?;
    table(&bus, 0)?;
    let addresses = pdma_model_software::control::ManagementAddresses {
        contexts: Some(BASE),
        ..ADDRESSES
    };
    publish(&memory, acquire(1), 1)?;
    let mut program = std::pin::pin!(run_at(&bus, memory.clone(), addresses));
    assert!(poll(program.as_mut()).is_pending());
    let consumer = ControlQueue::RESPONSE_OFFSET + ControlRingState::CONSUMER_OFFSET;
    write_word(&memory, consumer, 1)?;
    publish(
        &memory,
        ControlRequest {
            id: 2,
            opcode: ControlRequestOpcode::DestroyContext,
            ..acquire(2)
        },
        2,
    )?;
    bus.notification.raise();
    // When: the request observes a failed maintenance result.
    for _ in 0..4 {
        assert!(poll(program.as_mut()).is_pending());
    }
    // Then: firmware reports failure and processes a later reset without exiting.
    assert_eq!(
        (
            response(&memory)?.id,
            response(&memory)?.status,
            response(&memory)?.context
        ),
        (2, ControlResponseStatus::MaintenanceFailed, 0)
    );
    let valid = BASE
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
    write_word(&memory, consumer, 2)?;
    publish(
        &memory,
        ControlRequest {
            id: 3,
            opcode: ControlRequestOpcode::ResetChannel,
            ..acquire(3)
        },
        3,
    )?;
    bus.notification.raise();
    for _ in 0..4 {
        assert!(poll(program.as_mut()).is_pending());
    }
    let reset = u32::try_from(ChannelId::new(0).admin_offset(1).ok_or("reset")?)?;
    assert_eq!(
        bus.registers.lock().unwrap().get(&reset),
        Some(&1_u32.to_le_bytes().to_vec())
    );
    bus.registers
        .lock()
        .unwrap()
        .insert(reset, 0_u32.to_le_bytes().to_vec());
    for _ in 0..4 {
        assert!(poll(program.as_mut()).is_pending());
    }
    assert_eq!(
        (response(&memory)?.id, response(&memory)?.status),
        (3, ControlResponseStatus::Success)
    );
    Ok(())
}

#[tokio::test]
async fn context_owned_by_another_function_is_denied() -> TestResult {
    // Given: Function zero requests maintenance on Function one's live entry.
    let (bus, memory) = fixture()?;
    table(&bus, 1)?;
    publish(
        &memory,
        ControlRequest {
            opcode: ControlRequestOpcode::DestroyContext,
            ..acquire(9)
        },
        1,
    )?;
    let addresses = pdma_model_software::control::ManagementAddresses {
        contexts: Some(BASE),
        ..ADDRESSES
    };
    let mut program = std::pin::pin!(run_at(&bus, memory.clone(), addresses));
    // When: firmware validates the hardware-owned binding.
    assert!(poll(program.as_mut()).is_pending());
    // Then: no maintenance action is issued.
    assert_eq!(
        response(&memory)?.status,
        ControlResponseStatus::PermissionDenied
    );
    assert!(
        !bus.registers
            .lock()
            .unwrap()
            .contains_key(&(BASE + u32::try_from(ContextManagementRegisters::REQUEST.offset())?))
    );
    Ok(())
}

#[tokio::test]
async fn absent_context_hardware_returns_invalid_context() -> TestResult {
    // Given: ordinary managed assembly has no context table.
    let (bus, memory) = fixture()?;
    publish(
        &memory,
        ControlRequest {
            opcode: ControlRequestOpcode::CreateContext,
            ..acquire(4)
        },
        1,
    )?;
    let mut program = std::pin::pin!(run(&bus, memory.clone()));
    // When: a context operation arrives on its normal control ring.
    assert!(poll(program.as_mut()).is_pending());
    // Then: the service returns a typed failure without terminating.
    assert_eq!(
        (response(&memory)?.id, response(&memory)?.status),
        (4, ControlResponseStatus::InvalidContext)
    );
    Ok(())
}
