//! Notification visibility follows complete response publication.
/// Shared raw-bus test fixtures.
pub mod support;
use hardware_interface_pipeline::{Operation, RunOutcome, StepExecutor};
use hardware_interface_runtime::{MemoryAccess, MemoryAddress, MemoryAlignment, SharedMemory};
use pdma_model_contract::{DeviceRegisters, Response, ResponseRegisters, ResponseStatus};
use pdma_model_device::Device;
use pdma_model_device::{Pipeline, pipeline};
use support::{TestResult, command, read_word, setup, submit};
const RESPONSE_PRODUCER: usize =
    DeviceRegisters::RESPONSE.offset() + ResponseRegisters::PRODUCER.offset();

#[test]
fn irq_requires_published_response_on_every_step() -> TestResult {
    // Given
    let (mut device, memory) = setup(0x1100, 2)?;
    submit(&mut device, &memory, command())?;
    let mut hardware = pipeline();
    let mut operation = Operation::<Pipeline, Device<SharedMemory>>::new(());
    // When
    for _ in 0..32 {
        let outcome = StepExecutor::new()
            .step(&mut hardware, &mut operation, &mut device)
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
                    flags: pdma_model_contract::CompletionFlagsValues::default(),
                    id: 7,
                    status: ResponseStatus::Success,
                    bytes_completed: 4,
                }
            );
        }
        match outcome {
            RunOutcome::Progressed => {}
            RunOutcome::Pending(()) => return Err("unexpected stall".into()),
            RunOutcome::Complete(result) => {
                result?;
                assert!(irq);
                return Ok(());
            }
        }
    }
    Err("pipeline exceeded step bound".into())
}
