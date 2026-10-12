//! Abort effect boundaries through real memory and the sustained DMA pipeline.

use hardware_interface_pipeline::{Operation, StepOutcome};
use hardware_interface_runtime::{
    MemoryAccess, MemoryAccessError, MemoryAddress, MemoryAlignment, MemoryRegion, SharedMemory,
};
use pdma_model_contract::dma::{
    ChannelBlock, ChannelId, ChannelRegisters, Command, CommandRegisters, CompletionFlagsValues,
    ContextId, Endpoint, Response, ResponseRegisters, ResponseStatus, Segment,
};
use pdma_model_dma::{CommandLoop, Dma};
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
    pipeline: CommandLoop,
    operation: Operation<CommandLoop, Dma<ObservedMemory>>,
    normal: Command,
}

fn full_response_fixture() -> Result<Fixture, Box<dyn std::error::Error>> {
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
    let responses = base + ChannelBlock::RESPONSE.offset();
    for (offset, value) in [
        (base + CommandRegisters::BASE.offset(), 0x1000_u64),
        (responses + ResponseRegisters::BASE.offset(), 0x1100),
    ] {
        dma.write(offset, &value.to_le_bytes())?;
    }
    for offset in [
        base + CommandRegisters::CAPACITY.offset(),
        responses + ResponseRegisters::CAPACITY.offset(),
        control + ChannelRegisters::IRQ_ENABLE.offset(),
        control + ChannelRegisters::ENABLE.offset(),
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
    let normal = Command {
        copy_length: 4,
        id: 1,
        flags: CompletionFlagsValues { ioc: false },
        sg_context: 0,
        source: Endpoint::scatter_gather(0x1400, 1),
        destination: Endpoint::scatter_gather(0x1500, 1),
    };
    memory.write(0x1000.into(), MemoryAlignment::ONE, &normal.to_bytes()?)?;
    dma.write(
        base + CommandRegisters::PRODUCER.offset(),
        &1_u32.to_le_bytes(),
    )?;
    let mut pipeline = dma.command_loop();
    let mut operation = Operation::new(());
    for _ in 0..256 {
        assert!(matches!(
            operation.step(&mut pipeline, &mut dma)?,
            StepOutcome::InProgress
        ));
    }
    let copied: [u8; 4] = memory.read_typed(0x1700.into(), MemoryAlignment::ONE)?;
    assert_eq!(copied, [1, 3, 5, 7]);
    let response_bytes: pdma_model_contract::dma::ResponseRaw =
        memory.read_typed(0x1100.into(), MemoryAlignment::ONE)?;
    let response = Response::from_bytes(response_bytes.as_ref())?;
    assert_eq!((response.id, response.status), (1, ResponseStatus::Success));
    {
        let accesses = dma.memory.accesses.borrow();
        for access in [
            Access::Read(0x1400, Segment::RECORD_SIZE),
            Access::Read(0x1500, Segment::RECORD_SIZE),
            Access::Read(0x1600, 4),
            Access::Write(0x1700, 4),
            Access::Write(0x1100, Response::RECORD_SIZE),
        ] {
            assert!(
                accesses.contains(&access),
                "missing baseline access {access:?}"
            );
        }
    }
    assert!(dma.take_interrupt());
    Ok(Fixture {
        dma,
        memory,
        pipeline,
        operation,
        normal,
    })
}

#[test]
fn abort_with_full_response_ring_has_no_sg_payload_response_or_irq_effects()
-> Result<(), Box<dyn std::error::Error>> {
    // Given: a real observed SG copy has filled the only response slot.
    let Fixture {
        mut dma,
        memory,
        mut pipeline,
        mut operation,
        normal,
    } = full_response_fixture()?;
    let channel = ChannelId::new(0);
    let base = channel.register_offset(1).ok_or("channel layout")?;
    let responses = base + ChannelBlock::RESPONSE.offset();
    let target = Command {
        copy_length: normal.copy_length,
        id: 2,
        flags: CompletionFlagsValues { ioc: true },
        sg_context: 7,
        source: Endpoint {
            context: 7,
            ..normal.source
        },
        destination: Endpoint {
            context: 7,
            ..normal.destination
        },
    };
    memory.write(0x1600.into(), MemoryAlignment::ONE, &[2, 4, 6, 8])?;
    memory.write(0x1700.into(), MemoryAlignment::ONE, &[9; 4])?;
    memory.write(0x1000.into(), MemoryAlignment::ONE, &target.to_bytes()?)?;
    dma.write(
        base + CommandRegisters::PRODUCER.offset(),
        &2_u32.to_le_bytes(),
    )?;
    let mut before = [0; 4096];
    memory.read(0x1000.into(), MemoryAlignment::ONE, &mut before)?;
    let log_start = dma.memory.accesses.borrow().len();
    let mut producer_before = [0; 4];
    dma.read(
        responses + ResponseRegisters::PRODUCER.offset(),
        &mut producer_before,
    )?;
    assert_eq!(u32::from_le_bytes(producer_before), 1);
    // When: Abort arrives before target acceptance while response space remains full.
    dma.abort_context(ContextId::new(7), &[channel])?;
    assert_eq!(dma.context_status(ContextId::new(7)), Ok(false));
    for _ in 0..64 {
        assert!(matches!(
            operation.step(&mut pipeline, &mut dma)?,
            StepOutcome::InProgress
        ));
        if dma.context_status(ContextId::new(7)) == Ok(true) {
            break;
        }
    }
    // Then: only command-ring reads occur; discarded work publishes nothing and leaves no pending cleanup.
    assert_eq!(dma.context_status(ContextId::new(7)), Ok(true));
    let accesses = dma.memory.accesses.borrow();
    let new_accesses = accesses.get(log_start..).ok_or("access log bounds")?;
    assert!(!new_accesses.is_empty());
    assert!(
        new_accesses
            .iter()
            .all(|access| *access == Access::Read(0x1000, Command::RECORD_SIZE)),
        "unexpected Abort effects: {new_accesses:?}"
    );
    drop(accesses);
    let mut after = [0; 4096];
    memory.read(0x1000.into(), MemoryAlignment::ONE, &mut after)?;
    assert_eq!(after, before);
    let mut producer_after = [0; 4];
    dma.read(
        responses + ResponseRegisters::PRODUCER.offset(),
        &mut producer_after,
    )?;
    assert_eq!(producer_after, producer_before);
    let mut consumer = [0; 4];
    dma.read(base + CommandRegisters::CONSUMER.offset(), &mut consumer)?;
    assert_eq!(u32::from_le_bytes(consumer), 2);
    assert!(!dma.take_interrupt());
    Ok(())
}
