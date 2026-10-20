use super::support::{Fixture, Result, signal, wait};
use hardware_interface_runtime::{MemoryAccess, MemoryAddress, MemoryAlignment};
use pdma_model_contract::dma::{ChannelBlock, ChannelRegisters, SignalOperation};
use pdma_model_contract::dma::{
    Command, CopyCommand, Endpoint, SignalCommand, SignalOrder, SignalTarget,
};

#[test]
fn atomic_update_returns_old_value_once() -> Result {
    // Given: an initialized signal and one submitted update.
    let mut fixture = Fixture::new()?;
    fixture.store(12)?;
    fixture.submit(0, &[signal(101, SignalOperation::Add, 7)])?;
    // When: hardware runs well past response publication.
    fixture.tick(256)?;
    // Then: exactly one update and one observation are visible.
    assert_eq!(fixture.produced(0)?, 1);
    assert_eq!(fixture.value()?, 19);
    assert_eq!(fixture.response(0, 0)?.signal_value(), Some(12));
    assert_eq!(fixture.response(0, 0)?.id, 101);
    Ok(())
}

#[test]
fn waiting_channel_keeps_successor_order_and_allows_peer_signal() -> Result {
    // Given: a waiting channel with a successor and a peer that can satisfy it.
    let mut fixture = Fixture::new()?;
    fixture.submit(0, &[wait(1, 7), signal(2, SignalOperation::Add, 10)])?;
    fixture.tick(128)?;
    assert_eq!(fixture.produced(0)?, 0);
    assert_eq!(fixture.value()?, 0);
    assert_eq!(
        fixture.register(
            0,
            ChannelBlock::CONTROL.offset() + ChannelRegisters::FAULT_STATUS.offset()
        )?,
        0
    );
    // When: another channel publishes the awaited value.
    fixture.submit(1, &[signal(3, SignalOperation::Store, 7)])?;
    fixture.tick(512)?;
    // Then: the peer completes and the original channel retires in order without Resume.
    assert_eq!(fixture.produced(1)?, 1);
    assert_eq!(fixture.produced(0)?, 2);
    assert_eq!(fixture.response(0, 0)?.id, 1);
    assert_eq!(fixture.response(0, 0)?.signal_value(), Some(7));
    assert_eq!(fixture.response(0, 1)?.id, 2);
    assert_eq!(fixture.value()?, 17);
    Ok(())
}

#[test]
fn release_signal_and_acquire_wait_order_peer_payload_copies() -> Result {
    let mut fixture = Fixture::new()?;
    let payload = *b"published payload";
    fixture
        .memory
        .write(MemoryAddress::new(0x4100), MemoryAlignment::ONE, &payload)?;
    let copy = |id, source, destination| -> Result<Command> {
        Ok(Command::copy(
            id,
            CopyCommand {
                source: Endpoint::linear(source, 0),
                destination: Endpoint::linear(destination, 0),
                copy_length: u64::try_from(payload.len())?,
                sg_context: 0,
            },
        ))
    };
    let publish = |id, value| {
        Command::signal(
            id,
            SignalCommand {
                target: SignalTarget {
                    address: super::support::SIGNAL,
                    context: 1,
                },
                operation: SignalOperation::Store,
                order: SignalOrder::Release,
                value,
                expected: 0,
            },
        )
    };
    fixture.submit(0, &[wait(1, 1), copy(2, 0x4200, 0x4300)?, publish(3, 2)])?;
    fixture.submit(1, &[copy(4, 0x4100, 0x4200)?, publish(5, 1)])?;
    fixture.tick(1024)?;
    assert_eq!(fixture.produced(0)?, 3);
    assert_eq!(fixture.produced(1)?, 2);
    assert_eq!(fixture.value()?, 2);
    let mut observed = [0; 17];
    fixture.memory.read(
        MemoryAddress::new(0x4300),
        MemoryAlignment::ONE,
        &mut observed,
    )?;
    assert_eq!(observed, payload);
    assert_eq!(
        fixture.response(0, 1)?.bytes_completed(),
        u64::try_from(payload.len())?
    );
    Ok(())
}
