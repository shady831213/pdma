//! Completed local passes preserve the accepted hardware command and its grant.

/// Shared raw-bus fixtures.
pub mod support;

use hardware_interface_pipeline::{Operation, StepOutcome};
use hardware_interface_runtime::{MemoryAccess, MemoryAlignment};
use pdma_model_contract::dma::{ChannelBlock, CommandRegisters, Endpoint, ResponseRegisters};
use pdma_model_dma::COPY_CHUNK_BYTES;
use support::{TestResult, command, read_word, setup, submit};

#[test]
fn admission_and_completed_chunk_passes_preserve_descriptor_and_backpressure() -> TestResult {
    // Given: a command spanning three chunks and a later queued successor.
    const LENGTH: usize = COPY_CHUNK_BYTES * 2 + 88;
    let (mut device, memory) = setup(0x1100, 2)?;
    let descriptor = pdma_model_contract::dma::Command {
        copy_length: u64::try_from(LENGTH)?,
        source: Endpoint::linear(0x1400, 0),
        destination: Endpoint::linear(0x1800, 0),
        ..command()
    };
    memory.write(0x1400.into(), MemoryAlignment::ONE, &[0x5a; LENGTH])?;
    submit(&mut device, &memory, descriptor)?;
    let cmd = ChannelBlock::ALIGNMENT + ChannelBlock::COMMAND.offset();
    let resp = ChannelBlock::ALIGNMENT + ChannelBlock::RESPONSE.offset();
    let mut service = device.hardware();
    // When: the connected admission/execution graph completes each chunk invocation.
    for expected_prefix in [COPY_CHUNK_BYTES, COPY_CHUNK_BYTES * 2, LENGTH] {
        let mut operation = Operation::new(());
        for _ in 0..256 {
            if let StepOutcome::Complete(result) = operation.step(&mut service, &mut device)? {
                result?;
                break;
            }
        }
        assert!(operation.is_complete(), "local pass did not terminate");
        // Then: each pass commits only its chunk; later passes never refetch the descriptor.
        let mut bytes = [0; LENGTH];
        memory.read(0x1800.into(), MemoryAlignment::ONE, &mut bytes)?;
        let mut expected = [0; LENGTH];
        expected
            .get_mut(..expected_prefix)
            .ok_or("prefix")?
            .fill(0x5a);
        assert_eq!(bytes, expected);
        assert_eq!(
            read_word(&mut device, cmd + CommandRegisters::CONSUMER.offset())?,
            1
        );
        assert_eq!(
            read_word(&mut device, resp + ResponseRegisters::PRODUCER.offset())?,
            u32::from(expected_prefix == LENGTH)
        );
        memory.write(0x1000.into(), MemoryAlignment::ONE, &command().to_bytes()?)?;
        device.write(
            cmd + CommandRegisters::PRODUCER.offset(),
            &2_u32.to_le_bytes(),
        )?;
    }
    Ok(())
}
