use super::fault_memory::{Fault, fixture};
use super::support::{Fixture, Result, SIGNAL, offset, signal};
use pdma_model_contract::dma::{
    ChannelBlock, ChannelRegisters, Command, ResponseInfoValues, SignalCommand, SignalErrorReason,
    SignalOperation, SignalOrder, SignalTarget,
};

fn assert_error(response: pdma_model_contract::dma::Response, expected: SignalErrorReason) {
    assert!(
        matches!(response.info, ResponseInfoValues::SignalError(info) if info.reason == expected)
    );
    assert_eq!(response.signal_value(), None);
    assert_eq!(response.bytes_completed(), 0);
}

#[test]
fn invalid_orders_and_alignment_stop_without_modifying_signal() -> Result {
    for (operation, order, address) in [
        (SignalOperation::Load, SignalOrder::Release, SIGNAL),
        (SignalOperation::Load, SignalOrder::AcquireRelease, SIGNAL),
        (SignalOperation::Store, SignalOrder::Acquire, SIGNAL),
        (SignalOperation::Store, SignalOrder::AcquireRelease, SIGNAL),
        (SignalOperation::Add, SignalOrder::Relaxed, SIGNAL + 1),
    ] {
        // Given: an invalid operation followed by a valid same-channel update.
        let mut fixture = Fixture::new()?;
        fixture.store(41)?;
        fixture.submit(
            0,
            &[
                Command::signal(
                    1,
                    SignalCommand {
                        target: SignalTarget {
                            address,
                            context: 1,
                        },
                        operation,
                        order,
                        value: 99,
                        expected: 0,
                    },
                ),
                signal(2, SignalOperation::Add, 1),
            ],
        )?;
        // When: hardware runs beyond the stopping response.
        fixture.tick(256)?;
        // Then: invalid input neither changes the signal nor admits its successor.
        assert_eq!(fixture.produced(0)?, 1);
        assert_error(fixture.response(0, 0)?, SignalErrorReason::InvalidOperation);
        assert_eq!(fixture.value()?, 41);
    }
    Ok(())
}

#[test]
fn unsupported_and_terminal_access_errors_stop_without_fallback_writes() -> Result {
    for (fault, expected) in [
        (Fault::Unsupported, SignalErrorReason::Unsupported),
        (Fault::Access, SignalErrorReason::Access),
    ] {
        // Given: a backend rejecting indivisible access before any mutation.
        let mut fixture = fixture(fault)?;
        fixture.store(20)?;
        fixture.submit(
            0,
            &[
                signal(1, SignalOperation::Add, 7),
                signal(2, SignalOperation::Add, 9),
            ],
        )?;
        // When: the DMA attempts the atomic update.
        fixture.tick(256)?;
        // Then: exactly one stopping error appears with no ordinary-write fallback.
        assert_eq!(fixture.produced(0)?, 1);
        assert_error(fixture.response(0, 0)?, expected);
        assert_eq!(fixture.value()?, 20);
    }
    Ok(())
}

#[test]
fn atomic_page_fault_parks_then_resume_updates_exactly_once() -> Result {
    // Given: an atomic target whose platform translation is missing.
    let mut fixture = fixture(Fault::Page)?;
    fixture.store(30)?;
    fixture.submit(0, &[signal(1, SignalOperation::Add, 7)])?;
    // When: it faults, software repairs the mapping without resuming, then resumes.
    fixture.tick(256)?;
    assert_eq!(fixture.produced(0)?, 0);
    assert_eq!(fixture.value()?, 30);
    assert_eq!(
        fixture.register(
            0,
            ChannelBlock::CONTROL.offset() + ChannelRegisters::FAULT_STATUS.offset()
        )?,
        1
    );
    fixture.dma.memory.atomic_fault.set(Fault::None);
    fixture.tick(128)?;
    assert_eq!(fixture.produced(0)?, 0);
    assert_eq!(fixture.value()?, 30);
    fixture.dma.write(
        offset(0)? + ChannelBlock::CONTROL.offset() + ChannelRegisters::RESUME.offset(),
        &1_u32.to_le_bytes(),
    )?;
    fixture.tick(256)?;
    // Then: the original command produces one old-value response and one update.
    assert_eq!(fixture.produced(0)?, 1);
    assert_eq!(fixture.response(0, 0)?.signal_value(), Some(30));
    assert_eq!(fixture.value()?, 37);
    assert_eq!(
        fixture.register(
            0,
            ChannelBlock::CONTROL.offset() + ChannelRegisters::FAULT_STATUS.offset()
        )?,
        0
    );
    Ok(())
}

#[test]
fn response_write_failure_does_not_replay_committed_atomic_update() -> Result {
    // Given: signal memory is valid but the response destination rejects writes.
    let mut fixture = fixture(Fault::None)?;
    fixture.store(30)?;
    fixture.dma.memory.response_fault.set(true);
    fixture.submit(0, &[signal(1, SignalOperation::Add, 7)])?;
    // When: the update commits and completion publication fails.
    fixture.tick(256)?;
    // Then: the channel is fatal, no response was published, and the update occurs once.
    assert_eq!(fixture.produced(0)?, 0);
    assert_eq!(fixture.value()?, 37);
    assert_eq!(
        fixture.register(
            0,
            ChannelBlock::CONTROL.offset() + ChannelRegisters::FATAL.offset()
        )?,
        2
    );
    fixture.dma.memory.response_fault.set(false);
    fixture.tick(256)?;
    assert_eq!(fixture.value()?, 37);
    assert_eq!(fixture.produced(0)?, 0);
    Ok(())
}
