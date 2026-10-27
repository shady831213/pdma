//! Notification visibility follows complete event publication.

use pdma_model_contract::dma::{
    Command, CopyCommand, CopyCommandDestinationValues, CopyCommandSourceValues, ErrorInfo, Event,
    EventCode, EventInfoValues, EventRegisters, LinearEndpoint,
};

/// Shared raw-bus test fixtures.
pub mod support;
use hardware_interface_memory::{MemoryAccess, MemoryAddress, MemoryAlignment};
use hardware_interface_pipeline::{Stage, StepOutcome};
use support::{TestResult, read_word, setup, submit};
const EVENT_PRODUCER: usize = pdma_model_contract::dma::ChannelBlock::ALIGNMENT
    + pdma_model_contract::dma::ChannelBlock::EVENT.offset()
    + EventRegisters::PRODUCER.offset();

#[test]
fn irq_requires_published_event_on_every_step() -> TestResult {
    // Given
    let (mut device, memory) = setup(0x1100, 2)?;
    submit(
        &mut device,
        &memory,
        Command::Copy(CopyCommand {
            id: 7,
            source: CopyCommandSourceValues::Linear(LinearEndpoint {
                address: 0,
                offset: 0,
            }),
            destination: CopyCommandDestinationValues::Linear(LinearEndpoint {
                address: 0x1300,
                offset: 0,
            }),
            copy_length: 4,
            sg_context: 0,
            ..pdma_model_contract::dma::CopyCommand::ZERO
        }),
    )?;
    let mut hardware = support::chunk_pipeline();
    let mut state = Some(support::accept(
        &mut device,
        pdma_model_contract::dma::ChannelId::new(0),
    )?);
    // When
    for _ in 0..32 {
        let outcome = hardware
            .step(&mut device.execution_context(), &mut state)
            .map_err(|error| format!("{error:?}"))?;
        // Then
        let irq = device.irq();
        let published = read_word(&mut device, EVENT_PRODUCER)?;
        assert_eq!(irq, published == 1);
        if irq {
            let mut bytes = [0; Event::RECORD_SIZE];
            memory.read(MemoryAddress::new(0x1100), MemoryAlignment::ONE, &mut bytes)?;
            assert_eq!(
                Event::from_bytes(&bytes)?,
                Event {
                    id: 7,
                    code: EventCode::ReadError,
                    info: EventInfoValues::ReadError(ErrorInfo { bytes_completed: 0 }),
                }
            );
        }
        match outcome {
            StepOutcome::InProgress => {}
            StepOutcome::Complete(result) => {
                assert_eq!(
                    hardware_interface_pipeline::Operation::new(vec![(0, result?)])
                        .step(&mut pdma_model_dma::Retire, &mut device)?,
                    StepOutcome::Complete(Err(pdma_model_dma::Error::Halted))
                );
                assert!(irq);
                return Ok(());
            }
        }
    }
    Err("pipeline exceeded step bound".into())
}
