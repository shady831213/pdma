//! Transform access is bounded by the typed payload's immutable chunk geometry.

/// Shared hardware test fixtures.
pub mod support;

use hw_model_io::memory::SharedMemory;
use hw_model_io::memory::{MemoryAccess, MemoryAlignment};
use hw_model_pipeline::{Operation, Stage, StepOutcome, StepResult, Then};
use pdma_model_dma::{ExecutionContext, Payload, PrepareChunk, ReadCopy, WriteCopy};
use support::{TestResult, command, setup, submit};

struct FillChunk;

impl Stage<ExecutionContext<'_, SharedMemory>> for FillChunk {
    type Input = pdma_model_dma::Flow<Payload>;
    type Output = pdma_model_dma::Flow<Payload>;
    type Error = std::convert::Infallible;

    fn step(
        &mut self,
        _: &mut ExecutionContext<'_, SharedMemory>,
        input: &mut Option<Self::Input>,
    ) -> StepResult<Self::Output, Self::Error> {
        let Some(mut payload) = input.take() else {
            return Ok(StepOutcome::InProgress);
        };
        if let Some(data) = payload.data_mut() {
            match data.bytes_mut() {
                Ok(bytes) => {
                    assert_eq!(bytes.len(), 4);
                    bytes.fill(0x5a);
                }
                Err(error) => return Ok(StepOutcome::Complete(Ok(payload.stop(error)))),
            }
        }
        Ok(StepOutcome::Complete(Ok(payload)))
    }
}

#[test]
fn transform_can_modify_only_the_active_chunk_and_cannot_forge_its_length() -> TestResult {
    // Given: a four-byte command with adjacent destination sentinels.
    let (mut device, memory) = setup(0x1100, 2)?;
    memory.write(0x12ff.into(), MemoryAlignment::ONE, &[0xaa; 6])?;
    let descriptor = command();
    submit(&mut device, &memory, descriptor)?;
    let read = Then::new(
        PrepareChunk::default(),
        Then::new(ReadCopy::default(), FillChunk),
    );
    let pipeline = pdma_model_dma::CommandExecution::new(Then::new(read, WriteCopy::default()));
    let mut service = device.hardware_with(pipeline);
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
    assert_eq!(
        memory.read_buffer::<[u8; 6]>(0x12ff.into(), MemoryAlignment::ONE)?,
        [0xaa, 0x5a, 0x5a, 0x5a, 0x5a, 0xaa]
    );
    Ok(())
}
