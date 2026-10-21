//! Fixed producer frontiers and command-level context references.

/// Shared raw-bus fixtures.
pub mod support;

use hardware_interface_runtime::{MemoryAccess, MemoryAddress, MemoryAlignment};
use pdma_model_contract::dma::{
    ChannelBlock, ChannelId, CommandRegisters, ContextId, Response, ResponseRegisters,
};
use support::{TestResult, command, read_word, setup, submit};

#[test]
fn linear_command_does_not_reference_ignored_sg_selector() -> TestResult {
    // Given
    let (mut dma, memory) = setup(0x1100, 4)?;
    let mut target = command();
    target.source.context = 7;
    submit(&mut dma, &memory, target)?;
    let mut unrelated = command();
    unrelated.id = 8;
    unrelated.sg_context = 7;
    memory.write(
        MemoryAddress::new(0x1000 + u64::try_from(pdma_model_contract::dma::Command::RECORD_SIZE)?),
        MemoryAlignment::ONE,
        &unrelated.to_bytes()?,
    )?;
    memory.write(
        MemoryAddress::new(0x1200),
        MemoryAlignment::ONE,
        &[4, 3, 2, 1],
    )?;
    let base = ChannelId::new(0).register_offset(1).ok_or("layout")?;
    dma.write(
        base + CommandRegisters::PRODUCER.offset(),
        &2_u32.to_le_bytes(),
    )?;
    // When
    dma.abort_context(ContextId::new(7), &[ChannelId::new(0)])?;
    let mut pipeline = dma.hardware();
    let mut operation = hardware_interface_pipeline::HardwareDriver::new();
    for _ in 0..128 {
        let _ = operation.step(&mut pipeline, &mut dma);
    }
    // Then
    assert_eq!(dma.context_status(ContextId::new(7)), Ok(true));
    assert_eq!(
        read_word(
            &mut dma,
            base + ChannelBlock::RESPONSE.offset() + ResponseRegisters::PRODUCER.offset()
        )?,
        1
    );
    let mut response = [0; Response::RECORD_SIZE];
    memory.read(
        MemoryAddress::new(0x1100),
        MemoryAlignment::ONE,
        &mut response,
    )?;
    assert_eq!(Response::from_bytes(response.as_ref())?.id, unrelated.id);
    let mut bytes = [0; 4];
    memory.read(MemoryAddress::new(0x1300), MemoryAlignment::ONE, &mut bytes)?;
    assert_eq!(bytes, [4, 3, 2, 1]);
    Ok(())
}

#[test]
fn later_unrelated_publication_does_not_extend_the_drain_frontier() -> TestResult {
    // Given
    let (mut dma, memory) = setup(0x1100, 4)?;
    let mut target = command();
    target.source.context = 7;
    submit(&mut dma, &memory, target)?;
    dma.drain_context(ContextId::new(7), &[ChannelId::new(0)])?;
    let mut later = command();
    later.id = 8;
    memory.write(
        MemoryAddress::new(0x1000 + u64::try_from(pdma_model_contract::dma::Command::RECORD_SIZE)?),
        MemoryAlignment::ONE,
        &later.to_bytes()?,
    )?;
    let base = ChannelId::new(0).register_offset(1).ok_or("layout")?;
    dma.write(
        base + CommandRegisters::PRODUCER.offset(),
        &2_u32.to_le_bytes(),
    )?;
    let mut pipeline = dma.hardware();
    let mut operation = hardware_interface_pipeline::HardwareDriver::new();
    // When
    for _ in 0..128 {
        let _ = operation.step(&mut pipeline, &mut dma);
        if dma.context_status(ContextId::new(7)) == Ok(true) {
            break;
        }
    }
    // Then
    assert_eq!(dma.context_status(ContextId::new(7)), Ok(true));
    assert_eq!(
        read_word(&mut dma, base + CommandRegisters::CONSUMER.offset())?,
        1
    );
    assert_eq!(
        read_word(&mut dma, base + CommandRegisters::PRODUCER.offset())?,
        2
    );
    Ok(())
}
