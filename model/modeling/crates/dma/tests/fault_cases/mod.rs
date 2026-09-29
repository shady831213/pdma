mod boundaries;
mod lifecycle;
mod memory;

use self::memory::Paged;
use crate::support::{TestResult, configure, single_command};
use hardware_interface_pipeline::{Operation, RunOutcome, StepExecutor};
use hardware_interface_runtime::{MemoryAccess, MemoryAlignment, MemoryRegion, SharedMemory};
use pdma_model_contract::dma::{
    ChannelBlock, ChannelRegisters, Command, CommandRegisters, CompletionFlagsValues, Endpoint,
    PageFaultKindsAccess, Position, Response, ResponseRaw, ResponseRegisters, ResponseStatus,
    Segment,
};
use pdma_model_contract::dma::{CompletionInfo, ResponseInfoValues};
use pdma_model_dma::{Dma, RunCommand};

const CMD: usize = ChannelBlock::ALIGNMENT + ChannelBlock::COMMAND.offset();
const RESP: usize = ChannelBlock::ALIGNMENT + ChannelBlock::RESPONSE.offset();
const CTRL: usize = ChannelBlock::ALIGNMENT + ChannelBlock::CONTROL.offset();

fn setup() -> TestResult<Dma<Paged>> {
    let memory = SharedMemory::new();
    memory.add_regions(&[MemoryRegion::try_new(
        0x1000.into(),
        0x1000,
        MemoryAlignment::ONE,
    )?])?;
    configure(
        Paged {
            memory,
            missing: std::cell::Cell::new(None),
            faults: std::cell::Cell::new(0),
        },
        0x1100,
        2,
    )
}

fn read_word(device: &mut Dma<Paged>, offset: usize) -> TestResult<u32> {
    let mut bytes = [0; 4];
    device.read(offset, &mut bytes)?;
    Ok(u32::from_le_bytes(bytes))
}

fn run(device: &mut Dma<Paged>) -> TestResult {
    let mut stage = RunCommand::new(single_command(device));
    let mut operation = Operation::new(());
    for _ in 0..1024 {
        match StepExecutor::new()
            .step(&mut stage, &mut operation, device)
            .map_err(|e| format!("{e:?}"))?
        {
            RunOutcome::Complete(result) => {
                let _next = result?;
                return Ok(());
            }
            RunOutcome::Progressed => {}
            RunOutcome::Pending(()) => return Err("unexpected stall".into()),
        }
    }
    Err("command did not retire".into())
}

fn submit(device: &mut Dma<Paged>, command: Command, sequence: u32) -> TestResult<Response> {
    let slot = u64::from(sequence % 2);
    device.memory.memory.write(
        (0x1000 + slot * u64::try_from(Command::RECORD_SIZE)?).into(),
        MemoryAlignment::ONE,
        &command.to_bytes()?,
    )?;
    device.write(
        CMD + CommandRegisters::PRODUCER.offset(),
        &(sequence + 1).to_le_bytes(),
    )?;
    run(device)?;
    assert_eq!(
        read_word(device, RESP + ResponseRegisters::PRODUCER.offset())?,
        sequence + 1
    );
    let raw = device.memory.memory.read_typed::<ResponseRaw>(
        (0x1100 + slot * u64::try_from(Response::RECORD_SIZE)?).into(),
        MemoryAlignment::ONE,
    )?;
    Ok(Response::from_bytes(raw.as_ref())?)
}

fn consume(device: &mut Dma<Paged>, sequence: u32) -> TestResult {
    device.write(
        RESP + ResponseRegisters::CONSUMER.offset(),
        &(sequence + 1).to_le_bytes(),
    )?;
    Ok(())
}

fn streams(device: &Dma<Paged>) -> TestResult<Command> {
    for (address, bytes) in [
        (0x1200, b"abc".as_slice()),
        (0x1240, b"defgh"),
        (0x1300, b"--"),
        (0x1340, b"------"),
    ] {
        device
            .memory
            .memory
            .write(address.into(), MemoryAlignment::ONE, bytes)?;
    }
    for (table, entries) in [
        (0x1400, [(0x1200, 3), (0x1240, 5)]),
        (0x1500, [(0x1300, 2), (0x1340, 6)]),
    ] {
        for (index, (address, length)) in entries.into_iter().enumerate() {
            device.memory.memory.write(
                (table + u64::try_from(index * Segment::RECORD_SIZE)?).into(),
                MemoryAlignment::ONE,
                &Segment { address, length }.to_bytes()?,
            )?;
        }
    }
    Ok(Command {
        id: 7,
        copy_length: 8,
        source: Endpoint::scatter_gather(0x1400, 2),
        destination: Endpoint::scatter_gather(0x1500, 2),
        ..Command::ZERO
    })
}

#[test]
fn independent_sg_cursors_resume_after_repeated_read_and_write_faults() -> TestResult {
    for write in [false, true] {
        // Given: different SG boundaries and a missing byte inside the second payload chunk.
        let mut device = setup()?;
        let mut command = streams(&device)?;
        let missing = if write { 0x1343 } else { 0x1242 };
        device.memory.missing.set(Some((missing, write)));
        let first = submit(&mut device, command, 0)?;
        assert_eq!(first.status, ResponseStatus::PageFault);
        let first_fault = first.page_fault().ok_or("expected PageFault")?;
        assert_eq!(first.bytes_completed(), 3);
        assert_eq!(first_fault.fault_address, missing);
        assert_eq!(
            first_fault.kinds.access,
            if write {
                PageFaultKindsAccess::DestinationWrite
            } else {
                PageFaultKindsAccess::SourceRead
            }
        );
        assert_eq!(first_fault.source_next(), Position::scatter_gather(1, 0));
        assert_eq!(
            first_fault.destination_next(),
            Position::scatter_gather(1, 1)
        );
        assert_eq!(
            device
                .memory
                .memory
                .read_typed::<[u8; 6]>(0x1340.into(), MemoryAlignment::ONE)?,
            *b"c-----"
        );
        assert!(device.take_interrupt());
        assert_eq!(
            read_word(&mut device, CTRL + ChannelRegisters::IRQ_CAUSE.offset())?,
            1
        );
        consume(&mut device, 0)?;
        command.source = command
            .source
            .with_start(first_fault.source_next())
            .ok_or("position mode mismatch")?;
        command.destination = command
            .destination
            .with_start(first_fault.destination_next())
            .ok_or("position mode mismatch")?;
        command.copy_length -= first.bytes_completed();
        command.id = 8;
        // When: software retries before repair, then repairs and resubmits the same continuation.
        let repeated = submit(&mut device, command, 1)?;
        assert_eq!(repeated.status, ResponseStatus::PageFault);
        let repeated_fault = repeated.page_fault().ok_or("expected PageFault")?;
        assert_eq!(repeated.bytes_completed(), 0);
        assert_eq!(
            (
                repeated_fault.source_next(),
                repeated_fault.destination_next()
            ),
            (first_fault.source_next(), first_fault.destination_next())
        );
        consume(&mut device, 1)?;
        device.memory.missing.set(None);
        device
            .memory
            .memory
            .write(0x1300.into(), MemoryAlignment::ONE, b"ZZ")?;
        device
            .memory
            .memory
            .write(0x1340.into(), MemoryAlignment::ONE, b"Y")?;
        let done = submit(&mut device, command, 2)?;
        // Then: only the five uncommitted bytes are copied; the channel never needed reset.
        assert_eq!(
            done,
            Response {
                id: 8,
                info: ResponseInfoValues::Success(CompletionInfo { bytes_completed: 5 }),
                ..Response::ZERO
            }
        );
        assert_eq!(
            device
                .memory
                .memory
                .read_typed::<[u8; 2]>(0x1300.into(), MemoryAlignment::ONE)?,
            *b"ZZ"
        );
        assert_eq!(
            device
                .memory
                .memory
                .read_typed::<[u8; 6]>(0x1340.into(), MemoryAlignment::ONE)?,
            *b"Ydefgh"
        );
    }
    Ok(())
}
