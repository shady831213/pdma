//! Error publication belongs to the channel at the producing interaction.
#[doc = "Shared memory and MMIO fixtures."]
pub mod support;

use hardware_interface_pipeline::{Operation, Stage, StepOutcome, Then};
use hardware_interface_runtime::{MemoryAccess, MemoryAlignment};
use pdma_model_contract::dma::{
    ChannelBlock, ChannelId, ChannelRegisters, Endpoint, Event, EventCode, EventRegisters,
};
use pdma_model_dma::{PrepareChunk, ReadCopy, WriteCopy};
use support::{TestResult, accept, command, copy_mut, read_word, setup, submit};

#[test]
fn read_failure_publishes_before_any_downstream_stage_runs() -> TestResult {
    // Given: accepted Copy whose source is not mapped, with no event stage composed.
    let (mut dma, memory) = setup(0x1100, 2)?;
    let mut descriptor = command();
    copy_mut(&mut descriptor)?.source = Endpoint::linear(0x9000, 0);
    submit(&mut dma, &memory, descriptor)?;
    let input = accept(&mut dma, ChannelId::new(0))?;
    let mut read = Then::new(PrepareChunk::default(), ReadCopy::default());
    let mut output = None;
    // When: only the preparation and failing read interactions run.
    {
        let mut context = dma.execution_context();
        let mut operation = Operation::new(input);
        for _ in 0..64 {
            if let StepOutcome::Complete(result) = operation.step(&mut read, &mut context)? {
                output = Some(result?);
                break;
            }
        }
        assert!(operation.is_complete());
    }
    let producer =
        ChannelBlock::ALIGNMENT + ChannelBlock::EVENT.offset() + EventRegisters::PRODUCER.offset();
    let causes = ChannelBlock::ALIGNMENT
        + ChannelBlock::CONTROL.offset()
        + ChannelRegisters::IRQ_CAUSE.offset();
    // Then: the owner has already published exactly one real event and notified software.
    assert_eq!(read_word(&mut dma, producer)?, 1);
    assert_eq!(read_word(&mut dma, causes)?, 5);
    assert!(dma.irq());
    let mut bytes = [0; Event::RECORD_SIZE];
    memory.read(0x1100.into(), MemoryAlignment::ONE, &mut bytes)?;
    let event = Event::from_bytes(&bytes)?;
    assert_eq!(
        (event.id, event.code),
        (descriptor.id, EventCode::ReadError)
    );
    let mut write = WriteCopy::default();
    assert!(matches!(
        write.step(&mut dma.execution_context(), &mut output)?,
        StepOutcome::Complete(Ok(_))
    ));
    assert_eq!(read_word(&mut dma, producer)?, 1);
    Ok(())
}

#[test]
fn failing_read_reports_fatal_storage_failure_without_a_publisher() -> TestResult {
    let (mut dma, memory) = setup(0x9000, 2)?;
    let mut descriptor = command();
    copy_mut(&mut descriptor)?.source = Endpoint::linear(0xa000, 0);
    submit(&mut dma, &memory, descriptor)?;
    let mut input = Some(accept(&mut dma, ChannelId::new(0))?);
    let mut stage = Then::new(PrepareChunk::default(), ReadCopy::default());
    let mut finished = false;
    for _ in 0..64 {
        if let StepOutcome::Complete(result) =
            stage.step(&mut dma.execution_context(), &mut input)?
        {
            assert_eq!(
                result?.status(),
                Err(pdma_model_dma::Error::ChannelFatal(
                    pdma_model_dma::ChannelFatal::EventWrite
                ))
            );
            finished = true;
            break;
        }
    }
    assert!(finished);
    let control = ChannelBlock::ALIGNMENT + ChannelBlock::CONTROL.offset();
    assert_eq!(
        read_word(&mut dma, control + ChannelRegisters::IRQ_CAUSE.offset())?,
        12
    );
    assert_eq!(
        pdma_model_contract::dma::ChannelFatalValues::new(read_word(
            &mut dma,
            control + ChannelRegisters::FATAL.offset()
        )?)
        .code(),
        Ok(pdma_model_contract::dma::ChannelFatalCode::EventWrite)
    );
    assert_eq!(
        read_word(
            &mut dma,
            ChannelBlock::ALIGNMENT
                + ChannelBlock::EVENT.offset()
                + EventRegisters::PRODUCER.offset()
        )?,
        0
    );
    assert!(dma.take_interrupt());
    assert!(!dma.take_interrupt());
    Ok(())
}

#[test]
fn reset_between_chunk_preparation_and_read_prevents_error_publication() -> TestResult {
    use pdma_model_contract::dma::{DmaAdmin, DmaDiscovery};
    let (mut dma, memory) = setup(0x1100, 2)?;
    let mut descriptor = command();
    copy_mut(&mut descriptor)?.source = Endpoint::linear(0x9000, 0);
    submit(&mut dma, &memory, descriptor)?;
    let mut input = Some(accept(&mut dma, ChannelId::new(0))?);
    let mut prepare = PrepareChunk::default();
    let mut chunk = None;
    for _ in 0..64 {
        if let StepOutcome::Complete(result) =
            prepare.step(&mut dma.execution_context(), &mut input)?
        {
            chunk = Some(result?);
            break;
        }
    }
    assert!(
        chunk
            .as_ref()
            .and_then(pdma_model_dma::Flow::data)
            .is_some()
    );
    let reset = DmaDiscovery::EXTENT + DmaAdmin::CHANNELS_OFFSET;
    dma.write(reset, &1_u32.to_le_bytes())?;
    assert_eq!(read_word(&mut dma, reset)?, 1);
    let StepOutcome::Complete(result) =
        ReadCopy::default().step(&mut dma.execution_context(), &mut chunk)?
    else {
        return Err("stale read did not terminate".into());
    };
    let flow = result?;
    assert_eq!(flow.status(), Err(pdma_model_dma::Error::Reset));
    assert!(!dma.irq());
    let mut terminal = Some(vec![(0, flow.finish())]);
    assert_eq!(
        pdma_model_dma::Retire.step(&mut dma, &mut terminal)?,
        StepOutcome::Complete(Err(pdma_model_dma::Error::Reset))
    );
    assert_eq!(read_word(&mut dma, reset)?, 0);
    let mut bytes = [0; Event::RECORD_SIZE];
    memory.read(0x1100.into(), MemoryAlignment::ONE, &mut bytes)?;
    assert_eq!(bytes, [0; Event::RECORD_SIZE]);
    Ok(())
}
