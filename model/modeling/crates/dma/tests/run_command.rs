//! Custom command compositions retain the shared lifecycle boundary.

/// Shared raw-bus fixtures.
pub mod support;
mod run_command {
    mod retained;
}

use hardware_interface_pipeline::{AfterEach, Operation, Stage, StepOutcome};
use hardware_interface_runtime::{MemoryAccess, MemoryAddress, MemoryAlignment};
use pdma_model_contract::dma::{
    ChannelBlock, ChannelRegisters, CommandRegisters, ResponseRegisters,
};
use pdma_model_dma::{Error, ResetControl, RunCommand};
use std::ops::ControlFlow;
use support::{TestResult, command, read_word, setup, single_command, submit};

const CONTROL: usize =
    pdma_model_contract::dma::ChannelBlock::ALIGNMENT + ChannelBlock::CONTROL.offset();
const RESET: usize = pdma_model_contract::dma::DmaDiscovery::EXTENT
    + pdma_model_contract::dma::DmaAdmin::CHANNELS_OFFSET;
const CONSUMER: usize = pdma_model_contract::dma::ChannelBlock::ALIGNMENT
    + ChannelBlock::COMMAND.offset()
    + CommandRegisters::CONSUMER.offset();
const PRODUCER: usize = pdma_model_contract::dma::ChannelBlock::ALIGNMENT
    + ChannelBlock::RESPONSE.offset()
    + ResponseRegisters::PRODUCER.offset();

#[test]
fn decorated_pipeline_completes_through_run_command() -> TestResult {
    // Given: the real pipeline decorated with an independently composed reset controller.
    let (mut device, memory) = setup(0x1100, 2)?;
    memory.write(
        MemoryAddress::new(0x1200),
        MemoryAlignment::ONE,
        &[3, 1, 4, 1],
    )?;
    submit(&mut device, &memory, command())?;
    let pipeline = AfterEach::new(single_command(&device), ResetControl);
    let mut hardware = RunCommand::new(pipeline);
    let mut operation = Operation::new(());
    // When: the custom command is driven to its terminal boundary.
    let mut completed = false;
    for _ in 0..128 {
        match operation.step(&mut hardware, &mut device) {
            StepOutcome::Complete(result) => {
                assert_eq!(result?, ControlFlow::Continue(()));
                completed = true;
                break;
            }
            StepOutcome::Progressed => {}
            outcome @ (StepOutcome::Pending(()) | StepOutcome::Exhausted) => {
                return Err(format!("unexpected command outcome: {outcome:?}").into());
            }
        }
    }
    // Then: payload and response are published, and terminal ownership permits reset.
    assert!(completed);
    let mut bytes = [0; 4];
    memory.read(MemoryAddress::new(0x1300), MemoryAlignment::ONE, &mut bytes)?;
    assert_eq!(bytes, [3, 1, 4, 1]);
    assert_eq!(read_word(&mut device, PRODUCER)?, 1);
    device.write(RESET, &1_u32.to_le_bytes())?;
    let _ = ResetControl.step(&mut device, &mut ());
    assert_eq!(read_word(&mut device, RESET)?, 0);
    Ok(())
}

#[test]
fn decorated_pipeline_retires_reset_work_before_accepting_new_work() -> TestResult {
    // Given: accepted work whose downstream payload accesses have not started.
    let (mut device, memory) = setup(0x1100, 2)?;
    memory.write(MemoryAddress::new(0x1300), MemoryAlignment::ONE, &[9; 4])?;
    submit(&mut device, &memory, command())?;
    let pipeline = AfterEach::new(single_command(&device), ResetControl);
    let mut hardware = AfterEach::new(RunCommand::new(pipeline), ResetControl);
    let mut operation = Operation::new(());
    for _ in 0..32 {
        let _ = operation.step(&mut hardware, &mut device);
        if read_word(&mut device, CONSUMER)? == 1 {
            break;
        }
    }
    assert_eq!(read_word(&mut device, CONSUMER)?, 1);
    // When: software requests reset while the custom assembly retains accepted work.
    device.write(RESET, &1_u32.to_le_bytes())?;
    let _ = ResetControl.step(&mut device, &mut ());
    assert_eq!(read_word(&mut device, RESET)?, 1);
    assert_eq!(
        device.write(
            CONTROL + ChannelRegisters::ENABLE.offset(),
            &1_u32.to_le_bytes()
        ),
        Err(Error::ResetInProgress)
    );
    for _ in 0..128 {
        if matches!(
            operation.step(&mut hardware, &mut device),
            StepOutcome::Complete(Ok(_))
        ) {
            break;
        }
    }
    // Then: old work retires without payload or response writes; the same hardware runs new work.
    assert!(operation.is_complete());
    assert_eq!(read_word(&mut device, RESET)?, 0);
    assert_eq!(read_word(&mut device, PRODUCER)?, 0);
    let mut bytes = [0; 4];
    memory.read(MemoryAddress::new(0x1300), MemoryAlignment::ONE, &mut bytes)?;
    assert_eq!(bytes, [9; 4]);
    for (offset, value) in [
        (
            ChannelBlock::COMMAND.offset() + CommandRegisters::BASE.offset(),
            0x1000_u64,
        ),
        (
            ChannelBlock::RESPONSE.offset() + ResponseRegisters::BASE.offset(),
            0x1100_u64,
        ),
    ] {
        device.write(
            pdma_model_contract::dma::ChannelBlock::ALIGNMENT + offset,
            &value.to_le_bytes(),
        )?;
    }
    for (offset, value) in [
        (
            ChannelBlock::COMMAND.offset() + CommandRegisters::CAPACITY.offset(),
            2_u32,
        ),
        (
            ChannelBlock::RESPONSE.offset() + ResponseRegisters::CAPACITY.offset(),
            2_u32,
        ),
    ] {
        device.write(
            pdma_model_contract::dma::ChannelBlock::ALIGNMENT + offset,
            &value.to_le_bytes(),
        )?;
    }
    device.write(
        CONTROL + ChannelRegisters::ENABLE.offset(),
        &1_u32.to_le_bytes(),
    )?;
    memory.write(
        MemoryAddress::new(0x1200),
        MemoryAlignment::ONE,
        &[2, 7, 1, 8],
    )?;
    submit(&mut device, &memory, command())?;
    let mut next = Operation::new(());
    for _ in 0..128 {
        if let StepOutcome::Complete(result) = next.step(&mut hardware, &mut device) {
            assert_eq!(result?, ControlFlow::Continue(()));
            break;
        }
    }
    assert!(next.is_complete());
    memory.read(MemoryAddress::new(0x1300), MemoryAlignment::ONE, &mut bytes)?;
    assert_eq!(bytes, [2, 7, 1, 8]);
    assert_eq!(read_word(&mut device, PRODUCER)?, 1);
    Ok(())
}
