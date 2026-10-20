use super::{acquire, contexts::table, support::*};
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
async fn reset_remains_reachable_while_drain_is_pending() -> TestResult {
    reset_pending(ControlRequestOpcode::DrainContext)
}

#[tokio::test]
async fn reset_interrupting_destroy_preserves_context_validity() -> TestResult {
    reset_pending(ControlRequestOpcode::DestroyContext)
}

fn reset_pending(opcode: ControlRequestOpcode) -> TestResult {
    // Given: firmware owns a channel and hardware has not completed its Drain.
    let (bus, memory) = fixture()?;
    table(&bus, 0)?;
    set_result(&bus, ContextResultState::Pending)?;
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
            opcode,
            ..acquire(2)
        },
        2,
    )?;
    bus.notification.raise();
    for _ in 0..8 {
        assert!(poll(program.as_mut()).is_pending());
    }
    assert_eq!(
        read_word(
            &memory,
            ControlQueue::COMMAND_OFFSET + ControlRingState::CONSUMER_OFFSET
        )?,
        2
    );
    // When: PF sends recovery before the pending Drain produces a response.
    publish(
        &memory,
        ControlRequest {
            opcode: ControlRequestOpcode::ResetChannel,
            ..acquire(3)
        },
        3,
    )?;
    bus.notification.raise();
    for _ in 0..8 {
        assert!(poll(program.as_mut()).is_pending());
    }
    let reset = u32::try_from(ChannelId::new(0).admin_offset(1).ok_or("reset")?)?;
    // Then: firmware requests real reset, but does not report its success while busy.
    assert_eq!(
        bus.registers
            .lock()
            .map_err(|_| "register lock")?
            .get(&reset),
        Some(&1_u32.to_le_bytes().to_vec())
    );
    assert_eq!(
        read_word(
            &memory,
            ControlQueue::RESPONSE_OFFSET + ControlRingState::PRODUCER_OFFSET
        )?,
        1
    );
    set_result(&bus, ContextResultState::Failed)?;
    write_register(&bus, (reset, 0))?;
    for _ in 0..8 {
        assert!(poll(program.as_mut()).is_pending());
    }
    let first = response(&memory)?;
    write_word(&memory, consumer, 2)?;
    bus.notification.raise();
    for _ in 0..8 {
        assert!(poll(program.as_mut()).is_pending());
    }
    let second = response(&memory)?;
    let mut results = [(first.id, first.status), (second.id, second.status)];
    results.sort_by_key(|(id, _)| *id);
    assert_eq!(
        results,
        [
            (2, ControlResponseStatus::MaintenanceFailed),
            (3, ControlResponseStatus::Success)
        ]
    );
    let layout = ContextTableRegisters::for_channels(1, 1)?;
    let valid = BASE
        + u32::try_from(
            layout.entries().offset(0).ok_or("context entry")?
                + ContextEntryRegisters::VALID.offset(),
        )?;
    assert_eq!(
        bus.registers
            .lock()
            .map_err(|_| "register lock")?
            .get(&valid),
        Some(&1_u32.to_le_bytes().to_vec())
    );
    Ok(())
}

fn write_register(bus: &Bus, (address, value): (u32, u32)) -> TestResult {
    bus.registers
        .lock()
        .map_err(|_| "register lock")?
        .insert(address, value.to_le_bytes().to_vec());
    Ok(())
}

fn set_result(bus: &Bus, state: ContextResultState) -> TestResult {
    let address = BASE + u32::try_from(ContextManagementRegisters::RESULT.offset())?;
    write_register(
        bus,
        (address, ContextResultValues::RESET.with_state(state).raw()),
    )
}
