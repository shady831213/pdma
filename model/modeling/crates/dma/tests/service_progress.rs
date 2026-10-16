//! Finite service boundaries retain hardware traversal and accepted ownership.

/// Shared raw-bus test fixtures.
pub mod support;

use hardware_interface_pipeline::{Operation, StepOutcome};
mod service_progress {
    mod pending_tail;
}
use hardware_interface_runtime::{MemoryAccess, MemoryAlignment};
use pdma_model_contract::dma::{ChannelBlock, CommandRegisters, Endpoint, ResponseRegisters};
use pdma_model_dma::COPY_CHUNK_BYTES;
use support::{TestResult, command, read_word, setup, submit};

const CONSUMER: usize =
    ChannelBlock::ALIGNMENT + ChannelBlock::COMMAND.offset() + CommandRegisters::CONSUMER.offset();
const PRODUCER: usize = ChannelBlock::ALIGNMENT
    + ChannelBlock::RESPONSE.offset()
    + ResponseRegisters::PRODUCER.offset();

#[test]
fn finite_service_passes_complete_command_without_fetching_again() -> TestResult {
    // Given: a descriptor requiring three bounded copy interactions.
    let (mut device, memory) = setup(0x1100, 2)?;
    let mut descriptor = command();
    descriptor.copy_length = 600;
    descriptor.source = Endpoint::linear(0x1400, 0);
    descriptor.destination = Endpoint::linear(0x1800, 0);
    memory.write(0x1400.into(), MemoryAlignment::ONE, &[0x5a; 600])?;
    submit(&mut device, &memory, descriptor)?;
    let mut before = [0xff; 600];
    memory.read(0x1800.into(), MemoryAlignment::ONE, &mut before)?;
    assert_eq!(before, [0; 600]);
    assert_eq!(read_word(&mut device, PRODUCER)?, 0);
    let mut service = device.hardware();
    // When: finite invocations advance the retained command.
    let mut operation = Operation::new(());
    for _ in 0..256 {
        if let StepOutcome::Complete(result) = operation.step(&mut service, &mut device)? {
            result?;
            if read_word(&mut device, PRODUCER)? == 1 {
                break;
            }
            operation = Operation::new(());
        }
        if read_word(&mut device, CONSUMER)? == 1 {
            memory.write(0x1000.into(), MemoryAlignment::ONE, &command().to_bytes()?)?;
        }
    }
    // Then: all bytes commit and the descriptor is accepted exactly once.
    assert!(operation.is_complete());
    let mut bytes = [0; 600];
    memory.read(0x1800.into(), MemoryAlignment::ONE, &mut bytes)?;
    assert_eq!(bytes, [0x5a; 600]);
    assert_eq!(read_word(&mut device, CONSUMER)?, 1);
    assert_eq!(read_word(&mut device, PRODUCER)?, 1);
    Ok(())
}

#[test]
fn reset_between_chunks_retires_retained_progress() -> TestResult {
    // Given: accepted command state retained after its first chunk.
    let (mut device, memory) = setup(0x1100, 2)?;
    let mut descriptor = command();
    descriptor.copy_length = 600;
    descriptor.source = Endpoint::linear(0x1400, 0);
    descriptor.destination = Endpoint::linear(0x1800, 0);
    memory.write(0x1400.into(), MemoryAlignment::ONE, &[0x5a; 600])?;
    submit(&mut device, &memory, descriptor)?;
    let mut hardware = device.hardware();
    let mut operation = hardware_interface_pipeline::HardwareDriver::new();
    let mut committed = false;
    for _ in 0..256 {
        if let StepOutcome::Complete(result) = operation.step(&mut hardware, &mut device)? {
            result?;
        }
        if memory.read_typed::<[u8; 1]>(0x1800.into(), MemoryAlignment::ONE)? == [0x5a] {
            committed = true;
            break;
        }
    }
    assert!(committed);
    // When: reset arrives while hardware retains the command.
    let reset = pdma_model_contract::dma::DmaDiscovery::EXTENT
        + pdma_model_contract::dma::DmaAdmin::CHANNELS_OFFSET;
    device.write(reset, &1_u32.to_le_bytes())?;
    for _ in 0..256 {
        let _ = operation.step(&mut hardware, &mut device)?;
        if read_word(&mut device, reset)? == 0 {
            break;
        }
    }
    // Then: cleanup acknowledges reset without touching the remaining destination.
    assert_eq!(read_word(&mut device, reset)?, 0);
    let mut bytes = [0; 600];
    memory.read(0x1800.into(), MemoryAlignment::ONE, &mut bytes)?;
    let mut expected_bytes = [0; 600];
    expected_bytes
        .get_mut(..COPY_CHUNK_BYTES)
        .ok_or("prefix")?
        .fill(0x5a);
    assert_eq!(bytes, expected_bytes);
    assert_eq!(read_word(&mut device, PRODUCER)?, 0);
    Ok(())
}

#[test]
fn reset_at_every_chunk_boundary_retires_without_more_writes() -> TestResult {
    for prefix in 0..24 {
        // Given: one committed chunk and each subsequent local boundary.
        let (mut device, memory) = setup(0x1100, 2)?;
        let descriptor = pdma_model_contract::dma::Command {
            copy_length: u64::try_from(COPY_CHUNK_BYTES * 3)?,
            source: Endpoint::linear(0x1400, 0),
            destination: Endpoint::linear(0x1800, 0),
            ..command()
        };
        memory.write(
            0x1400.into(),
            MemoryAlignment::ONE,
            &[0x5a; COPY_CHUNK_BYTES * 3],
        )?;
        submit(&mut device, &memory, descriptor)?;
        let mut hardware = device.hardware();
        let mut next = hardware_interface_pipeline::HardwareDriver::new();
        let mut committed = false;
        for _ in 0..256 {
            if let StepOutcome::Complete(result) = next.step(&mut hardware, &mut device)? {
                result?;
            }
            if memory.read_typed::<[u8; 1]>(0x1800.into(), MemoryAlignment::ONE)? == [0x5a] {
                committed = true;
                break;
            }
        }
        assert!(committed);
        for _ in 0..prefix {
            if let StepOutcome::Complete(result) = next.step(&mut hardware, &mut device)? {
                result?;
            }
        }
        let mut before = [0; COPY_CHUNK_BYTES * 3];
        memory.read(0x1800.into(), MemoryAlignment::ONE, &mut before)?;
        // When: reset arrives before or after the traversal transfers to the next child.
        let reset = pdma_model_contract::dma::DmaDiscovery::EXTENT
            + pdma_model_contract::dma::DmaAdmin::CHANNELS_OFFSET;
        device.write(reset, &1_u32.to_le_bytes())?;
        for _ in 0..256 {
            let _ = next.step(&mut hardware, &mut device)?;
            if read_word(&mut device, reset)? == 0 {
                break;
            }
        }
        // Then: reset retires the accepted command and no old payload access occurs.
        assert!(!next.is_complete(), "prefix {prefix} hardware stopped");
        assert_eq!(read_word(&mut device, reset)?, 0, "prefix {prefix}");
        let mut after = [0; COPY_CHUNK_BYTES * 3];
        memory.read(0x1800.into(), MemoryAlignment::ONE, &mut after)?;
        assert_eq!(after, before, "prefix {prefix}");
    }
    Ok(())
}
