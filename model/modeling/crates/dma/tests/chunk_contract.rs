//! Transform access is bounded by the channel-owned active chunk.

/// Shared hardware test fixtures.
pub mod support;

use hardware_interface_pipeline::{Operation, RunError, Stage, StepOutcome, StepResult, Then};
use hardware_interface_runtime::{MemoryAccess, MemoryAlignment, SharedMemory};
use pdma_model_contract::dma::{CompletionInfo, Response, ResponseInfoValues, ResponseStatus};
use pdma_model_dma::{CheckRanges, Dma, Error, NextChunk, ReadCopy, Work, WriteCopy};
use support::{TestResult, command, setup, submit};

struct FillChunk;

impl Stage<Dma<SharedMemory>> for FillChunk {
    type Input = Option<Work>;
    type Output = Option<Work>;
    type Error = Error;
    type State = Option<Option<Work>>;

    fn start(input: Self::Input) -> Self::State {
        Some(input)
    }

    fn step(
        &mut self,
        device: &mut Dma<SharedMemory>,
        state: &mut Self::State,
    ) -> StepResult<Self::Output, Error> {
        let Some(work) = state.take() else {
            return Err(RunError::Exhausted);
        };
        if let Some(work) = &work {
            match device.work_scratch_mut(work) {
                Ok(Some(bytes)) => {
                    assert_eq!(bytes.len(), 4);
                    bytes.fill(0x5a);
                }
                Ok(None) => return Ok(StepOutcome::Complete(Err(Error::Routing))),
                Err(error) => return Ok(StepOutcome::Complete(Err(error))),
            }
        }
        Ok(StepOutcome::Complete(Ok(work)))
    }
}

#[test]
fn transform_can_modify_only_the_active_chunk_and_cannot_forge_its_length() -> TestResult {
    // Given: a four-byte command with adjacent destination sentinels.
    let (mut device, memory) = setup(0x1100, 2)?;
    memory.write(0x12ff.into(), MemoryAlignment::ONE, &[0xaa; 6])?;
    let descriptor = command();
    submit(&mut device, &memory, descriptor)?;
    let chunk = Then::new(
        Then::new(Then::new(NextChunk, ReadCopy), FillChunk),
        WriteCopy,
    );
    let pipeline = Then::new(Then::new(CheckRanges, chunk), device.responses());
    let mut service = device.service_with(pipeline);
    let mut operation = Operation::new(());
    // When: the transform fills its entire exposed slice and normal execution publishes it.
    for _ in 0..128 {
        if let StepOutcome::Complete(result) = operation.step(&mut service, &mut device)? {
            result?;
            break;
        }
    }
    // Then: the public slice cannot resize hardware scratch or change completion length.
    assert!(operation.is_complete());
    let raw = memory
        .read_typed::<pdma_model_contract::dma::ResponseRaw>(0x1100.into(), MemoryAlignment::ONE)?;
    assert_eq!(
        Response::from_bytes(raw.as_ref())?,
        Response {
            flags: descriptor.flags,
            id: descriptor.id,
            status: ResponseStatus::Success,
            info: ResponseInfoValues::Success(CompletionInfo {
                bytes_completed: descriptor.copy_length
            }),
        }
    );
    assert_eq!(
        memory.read_typed::<[u8; 6]>(0x12ff.into(), MemoryAlignment::ONE)?,
        [0xaa, 0x5a, 0x5a, 0x5a, 0x5a, 0xaa]
    );
    Ok(())
}
