//! Chunk boundaries preserve the accepted command within one hardware invocation.

use pdma_model_contract::dma::{
    ChannelBlock, CommandRegisters, CopyCommandDestinationValues, CopyCommandSourceValues,
    EventRegisters, LinearEndpoint,
};

/// Shared raw-bus fixtures.
pub mod support;

use hardware_interface_io::memory::{MemoryAccess, MemoryAlignment};
use hardware_interface_pipeline::{Operation, StepOutcome};
use pdma_model_dma::COPY_CHUNK_BYTES;
use support::{TestResult, command, read_word, setup, submit};

#[test]
fn command_continuation_preserves_descriptor_and_backpressure_between_chunks() -> TestResult {
    // Given: a command spanning three chunks and a later queued successor.
    const LENGTH: usize = COPY_CHUNK_BYTES * 2 + 88;
    let (mut device, memory) = setup(0x1100, 2)?;
    let descriptor = {
        let original = command();
        let pdma_model_contract::dma::Command::Copy(copy) = original else {
            return Err("copy fixture required".into());
        };
        pdma_model_contract::dma::Command::Copy(pdma_model_contract::dma::CopyCommand {
            id: original.id(),
            copy_length: u64::try_from(LENGTH)?,
            source: CopyCommandSourceValues::Linear(LinearEndpoint {
                address: 0x1400,
                offset: 0,
            }),
            destination: CopyCommandDestinationValues::Linear(LinearEndpoint {
                address: 0x1800,
                offset: 0,
            }),
            ..copy
        })
    };
    memory.write(0x1400.into(), MemoryAlignment::ONE, &[0x5a; LENGTH])?;
    submit(&mut device, &memory, descriptor)?;
    let cmd = ChannelBlock::ALIGNMENT + ChannelBlock::COMMAND.offset();
    let event = ChannelBlock::ALIGNMENT + ChannelBlock::EVENT.offset();
    let mut service = device.hardware();
    let mut operation = Operation::new(());
    // When: one command invocation reaches each observable committed chunk boundary.
    for expected_prefix in [COPY_CHUNK_BYTES, COPY_CHUNK_BYTES * 2, LENGTH] {
        for _ in 0..256 {
            if let StepOutcome::Complete(result) = operation.step(&mut service, &mut device)? {
                result?;
            }
            let address = 0x1800 + u64::try_from(expected_prefix - 1)?;
            if memory.read_buffer::<[u8; 1]>(address.into(), MemoryAlignment::ONE)? == [0x5a] {
                break;
            }
        }
        // Then: chunks commit in order without completing or readmitting the command.
        assert!(!operation.is_complete());
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
            read_word(&mut device, event + EventRegisters::PRODUCER.offset())?,
            0
        );
        memory.write(0x1000.into(), MemoryAlignment::ONE, &command().to_bytes()?)?;
        device.write(
            cmd + CommandRegisters::PRODUCER.offset(),
            &2_u32.to_le_bytes(),
        )?;
    }
    for _ in 0..256 {
        if let StepOutcome::Complete(result) = operation.step(&mut service, &mut device)? {
            result?;
            break;
        }
    }
    assert!(operation.is_complete());
    assert_eq!(
        read_word(&mut device, cmd + CommandRegisters::CONSUMER.offset())?,
        1
    );
    assert_eq!(
        read_word(&mut device, event + EventRegisters::PRODUCER.offset())?,
        0
    );
    Ok(())
}
