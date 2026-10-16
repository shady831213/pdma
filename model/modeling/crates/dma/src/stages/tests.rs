use crate::Dma;
use hardware_interface_pipeline::StepOutcome;

use hardware_interface_pipeline::Operation;
use hardware_interface_runtime::{
    MemoryAccess, MemoryAddress, MemoryAlignment, MemoryRegion, SharedMemory,
};
use pdma_model_contract::dma::{
    ChannelBlock, ChannelRegisters, Command, CommandRegisters, CompletionFlagsValues,
    CompletionInfo, Endpoint, Response, ResponseInfoValues, ResponseRegisters, ResponseStatus,
};

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

pub(super) fn queued_copy(response_base: u64, ioc: bool) -> TestResult<Dma<SharedMemory>> {
    let memory = SharedMemory::new();
    memory.add_regions(&[MemoryRegion::try_new(
        MemoryAddress::new(0x1000),
        0x1000,
        MemoryAlignment::ONE,
    )?])?;
    let command = Command {
        copy_length: 4,
        sg_context: 0,
        flags: CompletionFlagsValues { ioc },
        id: 17,
        source: Endpoint::linear(0x1200, 0),
        destination: Endpoint::linear(0x1300, 0),
    };
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
        (ChannelBlock::RESPONSE.offset(), response_base),
    ] {
        dma.write(ChannelBlock::ALIGNMENT + offset, &base.to_le_bytes())?;
    }
    for (offset, value) in [
        (
            ChannelBlock::COMMAND.offset() + CommandRegisters::CAPACITY.offset(),
            2_u32,
        ),
        (
            ChannelBlock::RESPONSE.offset() + ResponseRegisters::CAPACITY.offset(),
            2,
        ),
        (
            ChannelBlock::CONTROL.offset() + ChannelRegisters::IRQ_ENABLE.offset(),
            1,
        ),
        (
            ChannelBlock::CONTROL.offset() + ChannelRegisters::ENABLE.offset(),
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
    for ioc in [false, true] {
        for mask in 0..4_u32 {
            // Given: a real queued copy under every independent response-source mask.
            let mut dma = queued_copy(0x1100, ioc)?;
            dma.write(
                ChannelBlock::ALIGNMENT
                    + ChannelBlock::CONTROL.offset()
                    + ChannelRegisters::IRQ_MASK.offset(),
                &mask.to_le_bytes(),
            )?;
            // When: only the data path advances through publication.
            run_data_path(&mut dma)?;
            // Then: stored causes match the published response independently of delivery.
            let channel = dma.channels.first().ok_or("channel")?;
            assert_eq!(channel.response.pointers().producer, 1);
            assert_eq!(
                channel.registers.values.irq_cause.raw(),
                1 | (u32::from(ioc) << 1)
            );
            assert_eq!(
                dma.take_interrupt(),
                mask & 1 == 0 || (ioc && mask & 2 == 0)
            );
            let mut bytes = [0; Response::RECORD_SIZE];
            dma.memory
                .read(MemoryAddress::new(0x1100), MemoryAlignment::ONE, &mut bytes)?;
            assert_eq!(
                Response::from_bytes(&bytes)?,
                Response {
                    id: 17,
                    status: ResponseStatus::Success,
                    flags: CompletionFlagsValues { ioc },
                    info: ResponseInfoValues::Success(CompletionInfo { bytes_completed: 4 }),
                }
            );
            let mut copied = [0; 4];
            dma.memory.read(
                MemoryAddress::new(0x1300),
                MemoryAlignment::ONE,
                &mut copied,
            )?;
            assert_eq!(copied, [1, 3, 5, 7]);
        }
    }
    Ok(())
}

#[test]
fn fatal_refreshes_stored_cause_without_irq_stage_or_mmio_read() -> TestResult {
    // Given: accepted work whose response ring points outside mapped memory.
    let mut dma = queued_copy(0x3000, false)?;
    // When: the data path encounters a real response write failure.
    run_data_path(&mut dma)?;
    // Then: fatal status and its cause are already stored before any follow-up stage.
    let channel = dma.channels.first().ok_or("channel")?;
    assert_eq!(channel.registers.values.irq_cause.raw(), 8);
    assert_eq!(channel.response.pointers().producer, 0);
    assert_eq!(
        channel.registers.values.fatal.code(),
        Ok(pdma_model_contract::dma::ChannelFatalCode::ResponseWrite)
    );
    assert!(dma.take_interrupt());
    Ok(())
}

#[test]
fn consuming_response_refreshes_stored_cause_without_hardware_step() -> TestResult {
    // Given: a real published response with a retained nonempty cause.
    let mut dma = queued_copy(0x1100, false)?;
    run_data_path(&mut dma)?;
    assert_eq!(
        dma.channels
            .first()
            .ok_or("channel")?
            .registers
            .values
            .irq_cause
            .raw(),
        1
    );
    // When: software consumes the response without stepping hardware.
    dma.write(
        ChannelBlock::ALIGNMENT
            + ChannelBlock::RESPONSE.offset()
            + ResponseRegisters::CONSUMER.offset(),
        &1_u32.to_le_bytes(),
    )?;
    // Then: the owner has already cleared the stored queue cause.
    assert_eq!(
        dma.channels
            .first()
            .ok_or("channel")?
            .registers
            .values
            .irq_cause
            .raw(),
        0
    );
    Ok(())
}
