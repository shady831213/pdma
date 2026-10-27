use pdma_model_contract::dma::{
    ChannelBlock, ChannelConfiguration, ChannelInterrupts, ChannelRegisters, Command,
    CommandRegisters, CopyCommandDestinationValues, CopyCommandSourceValues, ErrorInfo, Event,
    EventCode, EventInfoValues, EventRegisters, LinearEndpoint,
};

use crate::Dma;
use hardware_interface_pipeline::StepOutcome;

use hardware_interface_io::memory::{MemoryAccess, MemoryAddress, MemoryAlignment};
use hardware_interface_io::memory::{MemoryRegion, SharedMemory};
use hardware_interface_pipeline::Operation;

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

#[test]
fn accepted_datapath_cannot_short_circuit_retirement_with_domain_errors() {
    fn assert_boundary<S>()
    where
        for<'a> S: hardware_interface_pipeline::Stage<
                crate::ExecutionContext<'a, SharedMemory>,
                Input = crate::Flow<crate::ChunkInput>,
                Output = crate::Flow<crate::Completion>,
                Error = std::convert::Infallible,
            >,
    {
    }
    assert_boundary::<crate::Pipeline>();
}

pub(super) fn queued_copy(event_base: u64, fail_read: bool) -> TestResult<Dma<SharedMemory>> {
    let memory = SharedMemory::new();
    memory.add_regions(&[MemoryRegion::try_new(
        MemoryAddress::new(0x1000),
        0x1000,
        MemoryAlignment::ONE,
    )?])?;
    let command = Command::Copy(pdma_model_contract::dma::CopyCommand {
        id: 17,
        copy_length: 4,
        sg_context: 0,
        source: CopyCommandSourceValues::Linear(LinearEndpoint {
            address: if fail_read { 0x3000 } else { 0x1200 },
            offset: 0,
        }),
        destination: CopyCommandDestinationValues::Linear(LinearEndpoint {
            address: 0x1300,
            offset: 0,
        }),
        ..pdma_model_contract::dma::CopyCommand::ZERO
    });
    memory.write(
        MemoryAddress::new(0x1000),
        MemoryAlignment::ONE,
        &command.to_bytes()?,
    )?;
    memory.write(
        MemoryAddress::new(0x1200),
        MemoryAlignment::ONE,
        &[1, 3, 5, 7],
    )?;
    let mut dma = Dma::new(memory, 1)?;
    for (offset, base) in [
        (ChannelBlock::COMMAND.offset(), 0x1000),
        (ChannelBlock::EVENT.offset(), event_base),
    ] {
        dma.write(ChannelBlock::ALIGNMENT + offset, &base.to_le_bytes())?;
    }
    for (offset, value) in [
        (
            ChannelBlock::COMMAND.offset() + CommandRegisters::CAPACITY.offset(),
            2_u32,
        ),
        (
            ChannelBlock::EVENT.offset() + EventRegisters::CAPACITY.offset(),
            2,
        ),
        (
            ChannelBlock::CONTROL.offset()
                + ChannelRegisters::IRQ.offset()
                + ChannelInterrupts::ENABLE.offset(),
            1,
        ),
        (
            ChannelBlock::CONTROL.offset()
                + ChannelRegisters::CONFIG.offset()
                + ChannelConfiguration::ENABLE.offset(),
            1,
        ),
        (
            ChannelBlock::COMMAND.offset() + CommandRegisters::PRODUCER.offset(),
            1,
        ),
    ] {
        dma.write(ChannelBlock::ALIGNMENT + offset, &value.to_le_bytes())?;
    }
    Ok(dma)
}

fn run_data_path(dma: &mut Dma<SharedMemory>) -> TestResult {
    let mut path = dma.hardware();
    let mut operation = Operation::new(());
    for _ in 0..1024 {
        if let StepOutcome::Complete(result) = operation.step(&mut path, dma)? {
            return Ok(result?);
        }
    }
    Err("data path exceeded step bound".into())
}

#[test]
fn publication_refreshes_stored_causes_without_irq_stage_or_mmio_read() -> TestResult {
    for mask in 0..4_u32 {
        // Given
        let mut dma = queued_copy(0x1100, true)?;
        dma.write(
            ChannelBlock::ALIGNMENT
                + ChannelBlock::CONTROL.offset()
                + ChannelRegisters::IRQ.offset()
                + ChannelInterrupts::MASK.offset(),
            &mask.to_le_bytes(),
        )?;
        // When
        run_data_path(&mut dma)?;
        // Then
        let channel = dma.channels.first().ok_or("channel")?;
        assert_eq!(channel.event.pointers().producer, 1);
        assert_eq!(channel.registers.values.irq.cause.raw(), 5);
        let bytes = dma
            .memory
            .read_buffer::<pdma_model_contract::dma::EventRaw>(
                0x1100.into(),
                MemoryAlignment::ONE,
            )?;
        assert_eq!(
            Event::from_bytes(bytes.as_ref())?,
            Event {
                id: 17,
                code: EventCode::ReadError,
                info: EventInfoValues::ReadError(ErrorInfo { bytes_completed: 0 })
            }
        );
        assert!(dma.take_interrupt());
    }
    Ok(())
}

#[test]
fn fatal_refreshes_stored_cause_without_irq_stage_or_mmio_read() -> TestResult {
    // Given
    let mut dma = queued_copy(0x3000, true)?;
    // When
    run_data_path(&mut dma)?;
    // Then
    let channel = dma.channels.first().ok_or("channel")?;
    assert_eq!(channel.registers.values.irq.cause.raw(), 12);
    assert_eq!(channel.event.pointers().producer, 0);
    assert_eq!(
        channel.registers.values.recovery.fatal.code(),
        Ok(pdma_model_contract::dma::ChannelFatalCode::EventWrite)
    );
    assert!(dma.take_interrupt());
    Ok(())
}

#[test]
fn consuming_event_clears_occupancy_but_retains_error_stop() -> TestResult {
    // Given
    let mut dma = queued_copy(0x1100, true)?;
    // When
    run_data_path(&mut dma)?;
    // Then
    assert_eq!(
        dma.channels
            .first()
            .ok_or("channel")?
            .registers
            .values
            .irq
            .cause
            .raw(),
        5
    );
    dma.write(
        ChannelBlock::ALIGNMENT + ChannelBlock::EVENT.offset() + EventRegisters::CONSUMER.offset(),
        &1_u32.to_le_bytes(),
    )?;
    assert_eq!(
        dma.channels
            .first()
            .ok_or("channel")?
            .registers
            .values
            .irq
            .cause
            .raw(),
        4
    );
    assert!(
        dma.command_request(pdma_model_contract::dma::ChannelId::new(0))?
            .is_none()
    );
    Ok(())
}

#[test]
fn successful_copy_does_not_require_event_capacity() -> TestResult {
    // Given
    let mut dma = queued_copy(0x1100, false)?;
    dma.channels
        .first_mut()
        .ok_or("channel")?
        .event
        .registers
        .values
        .producer = pdma_model_contract::register::ReadWordValues::new(2);
    // When
    run_data_path(&mut dma)?;
    // Then
    let copied = dma
        .memory
        .read_buffer::<[u8; 4]>(0x1300.into(), MemoryAlignment::ONE)?;
    assert_eq!(copied, [1, 3, 5, 7]);
    let channel = dma.channels.first().ok_or("channel")?;
    assert_eq!(channel.event.pointers().producer, 2);
    assert_eq!(
        channel.registers.values.recovery.fatal.code(),
        Ok(pdma_model_contract::dma::ChannelFatalCode::None)
    );
    assert!(channel.resident.is_none());
    Ok(())
}

#[test]
fn full_or_corrupt_event_ring_fails_error_publication_without_backpressure() -> TestResult {
    for (producer, expected) in [
        (2, pdma_model_contract::dma::ChannelFatalCode::EventFull),
        (
            3,
            pdma_model_contract::dma::ChannelFatalCode::EventConfiguration,
        ),
    ] {
        // Given
        let mut dma = queued_copy(0x1100, true)?;
        dma.channels
            .first_mut()
            .ok_or("channel")?
            .event
            .registers
            .values
            .producer = pdma_model_contract::register::ReadWordValues::new(producer);
        // When
        run_data_path(&mut dma)?;
        // Then
        let channel = dma.channels.first().ok_or("channel")?;
        assert_eq!(channel.registers.values.recovery.fatal.code(), Ok(expected));
        assert_eq!(channel.event.pointers().producer, producer);
        assert!(channel.registers.values.irq.cause.error());
        assert!(channel.resident.is_none());
        assert!(dma.take_interrupt());
    }
    Ok(())
}
