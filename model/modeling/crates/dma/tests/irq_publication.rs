//! Notification visibility follows complete response publication.
/// Shared raw-bus test fixtures.
pub mod support;
use hardware_interface_pipeline::{Operation, StepOutcome};
use hardware_interface_runtime::{MemoryAccess, MemoryAddress, MemoryAlignment};
use pdma_model_contract::dma::{CompletionInfo, ResponseInfoValues};
use pdma_model_contract::dma::{Response, ResponseRegisters, ResponseStatus};
use support::{TestResult, command, read_word, setup, submit};
const RESPONSE_PRODUCER: usize = pdma_model_contract::dma::ChannelBlock::ALIGNMENT
    + pdma_model_contract::dma::ChannelBlock::RESPONSE.offset()
    + ResponseRegisters::PRODUCER.offset();

#[test]
fn irq_requires_published_response_on_every_step() -> TestResult {
    // Given
    let (mut device, memory) = setup(0x1100, 2)?;
    submit(&mut device, &memory, command())?;
    let mut hardware = support::single_chunk(&device);
    let mut operation = Operation::new(pdma_model_contract::dma::ChannelId::new(0));
    // When
    for _ in 0..32 {
        let outcome = operation
            .step(&mut hardware, &mut device)
            .map_err(|error| format!("{error:?}"))?;
        // Then
        let irq = device.irq();
        let published = read_word(&mut device, RESPONSE_PRODUCER)?;
        assert_eq!(irq, published == 1);
        if irq {
            let mut bytes = [0; Response::RECORD_SIZE];
            memory.read(MemoryAddress::new(0x1100), MemoryAlignment::ONE, &mut bytes)?;
            assert_eq!(
                Response::from_bytes(&bytes)?,
                Response {
                    flags: pdma_model_contract::dma::CompletionFlagsValues::default(),
                    id: 7,
                    status: ResponseStatus::Success,
                    info: ResponseInfoValues::Success(CompletionInfo { bytes_completed: 4 }),
                }
            );
        }
        match outcome {
            StepOutcome::InProgress => {}
            StepOutcome::Complete(result) => {
                result?;
                assert!(irq);
                return Ok(());
            }
        }
    }
    Err("pipeline exceeded step bound".into())
}
