//! A stopping error drives IRQ; the producer publishes a complete diagnostic.

use pdma_model_contract::dma::{
    ChannelBlock, ChannelInterrupts, ChannelIrqCauseValues, ChannelRegisters, Command, CopyCommand,
    CopyCommandDestinationValues, CopyCommandSourceValues, ErrorInfo, Event, EventCode,
    EventInfoValues, EventRegisters, LinearEndpoint,
};

/// Shared raw-bus test fixtures.
pub mod support;
use hw_model_io::memory::{MemoryAccess, MemoryAddress, MemoryAlignment};
use hw_model_pipeline::{Stage, StepOutcome};
use support::{TestResult, read_word, setup, submit};
const EVENT_PRODUCER: usize = pdma_model_contract::dma::ChannelBlock::ALIGNMENT
    + pdma_model_contract::dma::ChannelBlock::EVENT.offset()
    + EventRegisters::PRODUCER.offset();

#[test]
fn stopping_irq_and_event_occupancy_follow_their_independent_causes() -> TestResult {
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
        let cause = ChannelIrqCauseValues::new(read_word(
            &mut device,
            ChannelBlock::ALIGNMENT
                + ChannelBlock::CONTROL.offset()
                + ChannelRegisters::IRQ.offset()
                + ChannelInterrupts::CAUSE.offset(),
        )?);
        assert_eq!(irq, cause.error() || cause.event_non_empty());
        assert_eq!(cause.event_non_empty(), published == 1);
        if published == 1 {
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
                    hw_model_pipeline::Operation::new(vec![(0, result?)])
                        .step(&mut pdma_model_dma::Retire, &mut device)?,
                    StepOutcome::Complete(Err(pdma_model_dma::Error::Halted))
                );
                assert!(irq && cause.error());
                assert_eq!(published, 1);
                return Ok(());
            }
        }
    }
    Err("pipeline exceeded step bound".into())
}
