//! A custom payload transform cannot forge completion or exceed its current chunk.

/// Shared hardware test fixtures.
pub mod support;

use hardware_interface_pipeline::{
    AfterEach, Operation, Repeat, RunError, Stage, StepExecutor, StepOutcome, StepResult, Then,
};
use hardware_interface_runtime::{MemoryAccess, MemoryAddress, MemoryAlignment, SharedMemory};
use pdma_model_contract::dma::{CompletionInfo, ResponseInfoValues};
use pdma_model_contract::dma::{Response, ResponseStatus};
use pdma_model_dma::{
    ChannelContext, CheckRanges, Error, NextChunk, ReadCopy, Resettable, Transfer, UpdateIrq,
    WriteCopy,
};
use support::{TestResult, command, setup, submit};

struct Resize(usize);
impl Stage<ChannelContext<'_, SharedMemory>> for Resize {
    type Input = Transfer;
    type Output = Transfer;
    type Error = Error;
    type State = Option<Transfer>;
    fn start(input: Transfer) -> Self::State {
        Some(input)
    }
    fn step(
        &mut self,
        _: &mut ChannelContext<'_, SharedMemory>,
        state: &mut Self::State,
    ) -> StepResult<Transfer, Error> {
        let Some(mut transfer) = state.take() else {
            return Err(RunError::Exhausted);
        };
        transfer.length = self.0;
        Ok(StepOutcome::Complete(Ok(transfer)))
    }
}

#[test]
fn invalid_transformed_chunk_does_not_write_or_claim_success() -> TestResult {
    for length in [0, 5] {
        // Given: a four-byte command with a transform changing the chunk length.
        let (mut device, memory) = setup(0x1100, 2)?;
        memory.write(MemoryAddress::new(0x1300), MemoryAlignment::ONE, &[0xaa; 8])?;
        submit(&mut device, &memory, command())?;
        let chunk = Then::new(
            Then::new(Then::new(NextChunk, ReadCopy), Resize(length)),
            WriteCopy,
        );
        let execution = Then::new(CheckRanges, Repeat::new(chunk));
        let mut pipeline = Then::new(
            Then::new(
                device.intake(),
                Resettable::new(AfterEach::new(execution, UpdateIrq)),
            ),
            device.responses(),
        );
        let mut operation = Operation::new(());
        let mut finished = false;
        // When: the transformed chunk reaches the ordinary write stage.
        for _ in 0..64 {
            match StepExecutor::new()
                .step(&mut pipeline, &mut operation, &mut device)
                .map_err(|e| format!("{e:?}"))?
            {
                StepOutcome::InProgress => {}
                StepOutcome::Complete(result) => {
                    result?;
                    finished = true;
                    break;
                }
            }
        }
        // Then: the command reports failure with no payload writes or false progress.
        assert!(finished);
        let raw = memory.read_typed::<pdma_model_contract::dma::ResponseRaw>(
            MemoryAddress::new(0x1100),
            MemoryAlignment::ONE,
        )?;
        assert_eq!(
            Response::from_bytes(raw.as_ref())?,
            Response {
                flags: pdma_model_contract::dma::CompletionFlagsValues::default(),
                id: 7,
                status: ResponseStatus::InvalidLength,
                info: ResponseInfoValues::InvalidLength(CompletionInfo { bytes_completed: 0 }),
            }
        );
        assert_eq!(
            memory.read_typed::<[u8; 8]>(MemoryAddress::new(0x1300), MemoryAlignment::ONE)?,
            [0xaa; 8]
        );
    }
    Ok(())
}
