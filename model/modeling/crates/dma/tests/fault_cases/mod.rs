mod boundaries;
mod chunks;
mod composition;
mod control;
mod lifecycle;
mod local;
mod memory;
mod peers;
mod resume;
mod tail;
mod termination;

use self::memory::Paged;
use crate::support::{TestResult, configure};
use hardware_interface_pipeline::{Operation, StepOutcome};
use hardware_interface_runtime::{MemoryAccess, MemoryAlignment, MemoryRegion, SharedMemory};
use pdma_model_contract::dma::{
    ChannelBlock, ChannelRegisters, Command, CommandRegisters, Endpoint, Response, ResponseRaw,
    ResponseRegisters, ResponseStatus, Segment,
};
use pdma_model_dma::{Dma, Hardware};

const CMD: usize = ChannelBlock::ALIGNMENT + ChannelBlock::COMMAND.offset();
const RESP: usize = ChannelBlock::ALIGNMENT + ChannelBlock::RESPONSE.offset();
const CTRL: usize = ChannelBlock::ALIGNMENT + ChannelBlock::CONTROL.offset();

struct Fixture {
    dma: Dma<Paged>,
    pipeline: Hardware,
    operation: hardware_interface_pipeline::HardwareDriver<Hardware, Dma<Paged>>,
}

impl std::ops::Deref for Fixture {
    type Target = Dma<Paged>;
    fn deref(&self) -> &Self::Target {
        &self.dma
    }
}

impl std::ops::DerefMut for Fixture {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.dma
    }
}

impl Fixture {
    fn until(
        &mut self,
        mut condition: impl FnMut(&mut Dma<Paged>) -> TestResult<bool>,
    ) -> TestResult {
        for _ in 0..1024 {
            if condition(&mut self.dma)? {
                return Ok(());
            }
            self.step()?;
        }
        Err("DMA condition was not reached before watchdog".into())
    }

    fn step(&mut self) -> TestResult {
        match self.operation.step(&mut self.pipeline, &mut self.dma)? {
            StepOutcome::InProgress => Ok(()),
            StepOutcome::Complete(result) => {
                result?;
                Err("loop stopped".into())
            }
        }
    }
}

fn setup() -> TestResult<Fixture> {
    let memory = SharedMemory::new();
    memory.add_regions(&[MemoryRegion::try_new(
        0x1000.into(),
        0x1000,
        MemoryAlignment::ONE,
    )?])?;
    let dma = configure(
        Paged {
            memory,
            missing: std::cell::Cell::new(None),
            faults: std::cell::Cell::new(0),
            accesses: std::cell::Cell::new(0),
            reads: std::cell::RefCell::new(Vec::new()),
        },
        0x1100,
        2,
    )?;
    let pipeline = dma.hardware();
    Ok(Fixture {
        dma,
        pipeline,
        operation: hardware_interface_pipeline::HardwareDriver::new(),
    })
}

fn read_word(device: &mut Dma<Paged>, offset: usize) -> TestResult<u32> {
    let mut bytes = [0; 4];
    device.read(offset, &mut bytes)?;
    Ok(u32::from_le_bytes(bytes))
}

fn fault_status(device: &mut Dma<Paged>) -> TestResult<u32> {
    read_word(device, CTRL + ChannelRegisters::FAULT_STATUS.offset())
}

fn resume(device: &mut Dma<Paged>, request: u32) -> TestResult {
    device.write(
        CTRL + ChannelRegisters::RESUME.offset(),
        &request.to_le_bytes(),
    )?;
    Ok(())
}

fn run(device: &mut Fixture) -> TestResult {
    for _ in 0..1024 {
        device.step()?;
    }
    Ok(())
}

fn submit(device: &mut Dma<Paged>, command: Command, sequence: u32) -> TestResult {
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
    Ok(())
}

fn response(device: &Dma<Paged>) -> TestResult<Response> {
    let raw = device
        .memory
        .memory
        .read_typed::<ResponseRaw>(0x1100.into(), MemoryAlignment::ONE)?;
    Ok(Response::from_bytes(raw.as_ref())?)
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
