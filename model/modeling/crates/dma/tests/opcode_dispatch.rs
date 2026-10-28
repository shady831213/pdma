//! Fetch preserves the accepted descriptor when resuming a yielded command.
use pdma_model_contract::dma::{FillCommand, FillCommandWidth, WordWrite};
use pdma_model_dma::COPY_CHUNK_BYTES;

use pdma_model_contract::dma::{
    ChannelBlock, ChannelId, Command, CommandHeader, CommandRegisters, Opcode, TrapCommand,
    WaitCommand, WaitCommandComparison, WordTarget, WordTargetWidth, WriteCommand,
};

/// Shared raw-bus fixtures.
pub mod support;

use hw_model_io::memory::{MemoryAccess, MemoryAddress, MemoryAlignment};
use hw_model_io::memory::{MemoryAccessError, SharedMemory};
use hw_model_pipeline::{Operation, StepOutcome};
use pdma_model_dma::{CommandRequest, Fetch};
use std::{cell::Cell, rc::Rc};
use support::TestResult;

#[derive(Debug)]
struct CountReads {
    memory: SharedMemory,
    descriptors: Rc<Cell<usize>>,
}

impl MemoryAccess for CountReads {
    type Error = MemoryAccessError;
    fn validate_with_sideband(
        &self,
        (): &(),
        address: MemoryAddress,
        alignment: MemoryAlignment,
        length: u64,
    ) -> Result<(), Self::Error> {
        self.memory.validate(address, alignment, length)
    }
    fn read_with_sideband(
        &self,
        (): &(),
        address: MemoryAddress,
        alignment: MemoryAlignment,
        bytes: &mut [u8],
    ) -> Result<(), Self::Error> {
        if address.raw() == 0x1000 {
            self.descriptors.set(self.descriptors.get() + 1);
        }
        self.memory.read(address, alignment, bytes)
    }
    fn write_with_sideband(
        &self,
        (): &(),
        address: MemoryAddress,
        alignment: MemoryAlignment,
        bytes: &[u8],
    ) -> Result<(), Self::Error> {
        self.memory.write(address, alignment, bytes)
    }
}

#[test]
fn resume_returns_original_descriptor_without_reading_replaced_ring_entry() -> TestResult {
    // Given: an accepted Wait has yielded, and its consumed ring slot is replaced.
    let (_, memory) = support::setup(0x1100, 2)?;
    let descriptors = Rc::new(Cell::new(0));
    let mut dma = support::configure(
        CountReads {
            memory: memory.clone(),
            descriptors: descriptors.clone(),
        },
        0x1100,
        2,
    )?;
    let command = Command::Wait(WaitCommand {
        header: CommandHeader {
            opcode: Opcode::Wait,
        },
        id: 41,
        target: WordTarget {
            address: 0x1200,
            context: 0,
            width: WordTargetWidth::Bits32,
        },
        value: 7,
        comparison: WaitCommandComparison::Equal,
    });
    memory.write(0x1000.into(), MemoryAlignment::ONE, &command.to_bytes()?)?;
    dma.write(
        ChannelBlock::ALIGNMENT
            + ChannelBlock::COMMAND.offset()
            + CommandRegisters::PRODUCER.offset(),
        &1_u32.to_le_bytes(),
    )?;
    let mut hardware = dma.hardware();
    let mut operation = Operation::new(());
    let mut finished = false;
    for _ in 0..1024 {
        if let StepOutcome::Complete(result) = operation.step(&mut hardware, &mut dma)? {
            result?;
            finished = true;
            break;
        }
    }
    assert!(finished, "waiting invocation must yield");
    assert_eq!(descriptors.get(), 1);
    memory.write(
        0x1000.into(),
        MemoryAlignment::ONE,
        &support::command().to_bytes()?,
    )?;
    let request = dma
        .command_request(ChannelId::new(0))?
        .ok_or("resume request")?;
    assert!(matches!(request, CommandRequest::Resume(_)));

    // When: Fetch accepts the resumed task.
    let StepOutcome::Complete(result) = Operation::new(Some(request)).step(&mut Fetch, &mut dma)?
    else {
        return Err("resume did not complete Fetch".into());
    };
    let flow = result?;

    // Then: dispatch receives the immutable accepted descriptor without ring I/O.
    assert_eq!(flow.data().map(|fetched| *fetched.command()), Some(command));
    assert_eq!(
        flow.data().map(pdma_model_dma::FetchedCommand::admission),
        Some(pdma_model_dma::CommandAdmission::Resume)
    );
    assert_eq!(descriptors.get(), 1);
    Ok(())
}

#[test]
fn fill_retains_one_invocation_and_backpressures_successor_between_chunks() -> TestResult {
    // Given: a multi-chunk Fill followed by a visible Write successor.
    const LENGTH: usize = COPY_CHUNK_BYTES * 2 + 11;
    let (mut dma, memory) = support::setup(0x1100, 2)?;
    let command = Command::Fill(FillCommand {
        header: CommandHeader {
            opcode: Opcode::Fill,
        },
        id: 21,
        address: 0x1400,
        length: u64::try_from(LENGTH)?,
        pattern: 0x5a,
        context: 0,
        width: FillCommandWidth::Byte,
    });
    let successor = Command::Write(WriteCommand {
        header: CommandHeader {
            opcode: Opcode::Write,
        },
        id: 22,
        word: WordWrite {
            target: WordTarget {
                address: 0x1900,
                context: 0,
                width: WordTargetWidth::Bits32,
            },
            value: 7,
        },
    });
    support::submit(&mut dma, &memory, command)?;
    memory.write(
        (0x1000 + u64::try_from(Command::RECORD_SIZE)?).into(),
        MemoryAlignment::ONE,
        &successor.to_bytes()?,
    )?;
    let ring = ChannelBlock::ALIGNMENT + ChannelBlock::COMMAND.offset();
    dma.write(
        ring + CommandRegisters::PRODUCER.offset(),
        &2_u32.to_le_bytes(),
    )?;
    let mut hardware = dma.hardware();
    let mut operation = Operation::new(());
    // When: the same invocation reaches each committed Fill prefix.
    for prefix in [COPY_CHUNK_BYTES, COPY_CHUNK_BYTES * 2, LENGTH] {
        for _ in 0..256 {
            if let StepOutcome::Complete(result) = operation.step(&mut hardware, &mut dma)? {
                result?;
            }
            if memory.read_buffer::<[u8; 1]>(
                (0x1400 + u64::try_from(prefix - 1)?).into(),
                MemoryAlignment::ONE,
            )? == [0x5a]
            {
                break;
            }
        }
        // Then: its exact prefix commits without admission of the queued successor.
        assert!(!operation.is_complete());
        let mut bytes = [0; LENGTH];
        memory.read(0x1400.into(), MemoryAlignment::ONE, &mut bytes)?;
        let mut expected = [0; LENGTH];
        expected.get_mut(..prefix).ok_or("prefix")?.fill(0x5a);
        assert_eq!(bytes, expected);
        assert_eq!(
            support::read_word(&mut dma, ring + CommandRegisters::CONSUMER.offset())?,
            1
        );
        assert_eq!(
            memory.read_buffer::<[u8; 4]>(0x1900.into(), MemoryAlignment::ONE)?,
            [0; 4]
        );
        memory.write(
            0x1000.into(),
            MemoryAlignment::ONE,
            &Command::Trap(TrapCommand {
                header: CommandHeader {
                    opcode: Opcode::Trap,
                },
                id: 99,
            })
            .to_bytes()?,
        )?;
    }
    for _ in 0..256 {
        if let StepOutcome::Complete(result) = operation.step(&mut hardware, &mut dma)? {
            result?;
            break;
        }
    }
    assert!(operation.is_complete());
    assert_eq!(
        support::read_word(&mut dma, ring + CommandRegisters::CONSUMER.offset())?,
        1
    );
    support::run(&mut dma)?;
    assert_eq!(
        memory.read_buffer::<[u8; 4]>(0x1900.into(), MemoryAlignment::ONE)?,
        7_u32.to_le_bytes()
    );
    support::assert_no_event(&mut dma)?;
    Ok(())
}
