//! Abort effect boundaries through real memory and the sustained DMA pipeline.

use pdma_model_contract::dma::{
    ChannelBlock, ChannelConfiguration, ChannelId, ChannelInterrupts, ChannelRegisters, Command,
    CommandRegisters, ContextId, CopyCommandDestinationValues, CopyCommandSourceValues,
    EventRegisters, ScatterGatherEndpoint, Segment,
};

use hardware_interface_memory::{
    MemoryAccess, MemoryAccessError, MemoryAddress, MemoryAlignment, MemoryRegion, SharedMemory,
};
use hardware_interface_pipeline::StepOutcome;
use pdma_model_dma::{Dma, Hardware};
use std::cell::RefCell;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Access {
    Validate(u64, u64),
    Read(u64, usize),
    Write(u64, usize),
}

struct ObservedMemory {
    backing: SharedMemory,
    accesses: RefCell<Vec<Access>>,
}

impl MemoryAccess for ObservedMemory {
    type Error = MemoryAccessError;
    fn validate_with_sideband(
        &self,
        (): &(),
        address: MemoryAddress,
        alignment: MemoryAlignment,
        length: u64,
    ) -> Result<(), MemoryAccessError> {
        self.accesses
            .borrow_mut()
            .push(Access::Validate(address.raw(), length));
        self.backing.validate(address, alignment, length)
    }

    fn read_with_sideband(
        &self,
        (): &(),
        address: MemoryAddress,
        alignment: MemoryAlignment,
        bytes: &mut [u8],
    ) -> Result<(), MemoryAccessError> {
        self.accesses
            .borrow_mut()
            .push(Access::Read(address.raw(), bytes.len()));
        self.backing.read(address, alignment, bytes)
    }

    fn write_with_sideband(
        &self,
        (): &(),
        address: MemoryAddress,
        alignment: MemoryAlignment,
        bytes: &[u8],
    ) -> Result<(), MemoryAccessError> {
        self.accesses
            .borrow_mut()
            .push(Access::Write(address.raw(), bytes.len()));
        self.backing.write(address, alignment, bytes)
    }
}

struct Fixture {
    dma: Dma<ObservedMemory>,
    memory: SharedMemory,
    pipeline: Hardware,
    operation: hardware_interface_pipeline::HardwareDriver<Hardware, Dma<ObservedMemory>>,
    normal: Command,
}

fn retired_copy_fixture() -> Result<Fixture, Box<dyn std::error::Error>> {
    let memory = SharedMemory::new();
    memory.add_regions(&[MemoryRegion::try_new(
        0x1000.into(),
        0x1000,
        MemoryAlignment::ONE,
    )?])?;
    let mut dma = Dma::new(
        ObservedMemory {
            backing: memory.clone(),
            accesses: RefCell::default(),
        },
        1,
    )?;
    let channel = ChannelId::new(0);
    let base = channel.register_offset(1).ok_or("channel layout")?;
    let control = base + ChannelBlock::CONTROL.offset();
    let events = base + ChannelBlock::EVENT.offset();
    for (offset, value) in [
        (base + CommandRegisters::BASE.offset(), 0x1000_u64),
        (events + EventRegisters::BASE.offset(), 0x1100),
    ] {
        dma.write(offset, &value.to_le_bytes())?;
    }
    for offset in [
        base + CommandRegisters::CAPACITY.offset(),
        events + EventRegisters::CAPACITY.offset(),
        control + ChannelRegisters::IRQ.offset() + ChannelInterrupts::ENABLE.offset(),
        control + ChannelRegisters::CONFIG.offset() + ChannelConfiguration::ENABLE.offset(),
    ] {
        dma.write(offset, &1_u32.to_le_bytes())?;
    }
    for (table, payload) in [(0x1400, 0x1600), (0x1500, 0x1700)] {
        memory.write(
            MemoryAddress::new(table),
            MemoryAlignment::ONE,
            &Segment {
                address: payload,
                length: 4,
            }
            .to_bytes()?,
        )?;
    }
    memory.write(0x1600.into(), MemoryAlignment::ONE, &[1, 3, 5, 7])?;
    let normal = Command::Copy(pdma_model_contract::dma::CopyCommand {
        id: 1,
        copy_length: 4,
        sg_context: 0,
        source: CopyCommandSourceValues::ScatterGather(ScatterGatherEndpoint {
            list_address: 0x1400,
            count: 1,
            index: 0,
            offset: 0,
        }),
        destination: CopyCommandDestinationValues::ScatterGather(ScatterGatherEndpoint {
            list_address: 0x1500,
            count: 1,
            index: 0,
            offset: 0,
        }),
        ..pdma_model_contract::dma::CopyCommand::ZERO
    });
    memory.write(0x1000.into(), MemoryAlignment::ONE, &normal.to_bytes()?)?;
    dma.write(
        base + CommandRegisters::PRODUCER.offset(),
        &1_u32.to_le_bytes(),
    )?;
    let mut pipeline = dma.hardware();
    let mut operation = hardware_interface_pipeline::HardwareDriver::new();
    for _ in 0..256 {
        assert!(matches!(
            operation.step(&mut pipeline, &mut dma)?,
            StepOutcome::InProgress
        ));
    }
    let copied: [u8; 4] = memory.read_typed(0x1700.into(), MemoryAlignment::ONE)?;
    assert_eq!(copied, [1, 3, 5, 7]);
    {
        let accesses = dma.memory.accesses.borrow();
        for access in [
            Access::Read(0x1400, Segment::RECORD_SIZE),
            Access::Read(0x1500, Segment::RECORD_SIZE),
            Access::Read(0x1600, 4),
            Access::Write(0x1700, 4),
        ] {
            assert!(
                accesses.contains(&access),
                "missing baseline access {access:?}"
            );
        }
    }
    assert!(!dma.take_interrupt());
    Ok(Fixture {
        dma,
        memory,
        pipeline,
        operation,
        normal,
    })
}

mod context_abort_effects {
    use super::*;
    mod retired;
}
