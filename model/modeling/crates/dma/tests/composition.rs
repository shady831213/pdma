//! A downstream stage can transform the copy payload without replacing execution.

/// Shared raw-bus test fixtures.
pub mod support;

use hardware_interface_pipeline::{
    AfterEach, Operation, Repeat, RunOutcome, Stage, StepExecutor, StepOutcome, Then,
};
use hardware_interface_runtime::{MemoryAccess, MemoryAddress, MemoryAlignment, SharedMemory};
use pdma_model_dma::{
    ChannelContext, CheckLengths, Error, NextChunk, ReadCopy, Resettable, Transfer, UpdateIrq,
    WriteCopy,
};
use support::{TestResult, command, setup, submit};

struct InvertPayload;

impl Stage<ChannelContext<'_, SharedMemory>> for InvertPayload {
    type Input = Transfer;
    type Output = Transfer;
    type Error = Error;
    type Wait = ();
    type State = Option<Transfer>;
    fn start(input: Transfer) -> Self::State {
        Some(input)
    }
    fn step(
        &mut self,
        _: &mut ChannelContext<'_, SharedMemory>,
        state: &mut Self::State,
    ) -> StepOutcome<Transfer, Error, ()> {
        let Some(mut transfer) = state.take() else {
            return StepOutcome::Exhausted;
        };
        for byte in &mut transfer.bytes {
            *byte = !*byte;
        }
        StepOutcome::Complete(Ok(transfer))
    }
}

#[test]
fn inserted_transform_changes_payload_without_new_executor_or_device_core() -> TestResult {
    // Given
    let (mut device, memory) = setup(0x1100, 2)?;
    memory.write(
        MemoryAddress::new(0x1200),
        MemoryAlignment::ONE,
        &[0, 1, 2, 255],
    )?;
    submit(&mut device, &memory, command())?;
    let chunk = Then::new(
        Then::new(Then::new(NextChunk, ReadCopy), InvertPayload),
        WriteCopy,
    );
    let execution = Then::new(CheckLengths, Repeat::new(chunk));
    let mut hardware = Then::new(
        Then::new(
            device.intake(),
            Resettable::new(AfterEach::new(execution, UpdateIrq)),
        ),
        device.responses(),
    );
    let mut operation = Operation::new(());
    let mut complete = false;
    // When
    for _ in 0..32 {
        match StepExecutor::new()
            .step(&mut hardware, &mut operation, &mut device)
            .map_err(|e| format!("{e:?}"))?
        {
            RunOutcome::Progressed => {}
            RunOutcome::Pending(()) => return Err("unexpected stall".into()),
            RunOutcome::Complete(result) => {
                result?;
                complete = true;
                break;
            }
        }
    }
    // Then
    let mut bytes = [0; 4];
    memory.read(MemoryAddress::new(0x1300), MemoryAlignment::ONE, &mut bytes)?;
    assert!(complete);
    assert_eq!(bytes, [255, 254, 253, 0]);
    assert!(device.irq());
    Ok(())
}

#[test]
fn independently_composed_execution_obeys_device_fatal() -> TestResult {
    // Given: an execution stage started with a valid command.
    let (mut device, _) = setup(0x1100, 2)?;
    let mut execution = CheckLengths;
    let mut operation = Operation::new(command());
    device.signal_fatal();
    // When: the stage is driven without the default outer pipeline.
    let mut context = device.channel_context(pdma_model_contract::ChannelId::new(0))?;
    let result = operation.step(&mut execution, &mut context);
    // Then: device fatal blocks even independently composed hardware.
    assert!(matches!(result, StepOutcome::Complete(Err(Error::Halted))));
    Ok(())
}
