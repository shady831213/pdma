use super::support::{CAUSE, EVENT_PRODUCER, Rig, TestResult, fence, target};
use hardware_interface_runtime::{
    AtomicAccessError, AtomicOperation, MemoryAccess, MemoryAddress, MemoryAlignment, SharedMemory,
};
use pdma_model_contract::dma::ChannelRecovery;
use pdma_model_contract::dma::{
    AtomicCommand, AtomicCommandOperandsAddValues, AtomicCommandOperandsValues, AtomicCommandOrder,
    CapabilitiesValues, ChannelBlock, ChannelRegisters, Command, DmaAdmin, DmaDiscovery, WordWrite,
};
use pdma_model_dma::{Dma, DmaConfig, MemoryFailure};
use std::{cell::Cell, rc::Rc};
mod recovery;
mod timestamp;

struct FaultMemory {
    memory: SharedMemory,
    fault: Rc<Cell<bool>>,
    address: u64,
    commits: Rc<Cell<usize>>,
}

impl MemoryAccess for FaultMemory {
    type Error = MemoryFailure;
    fn validate_with_sideband(
        &self,
        (): &(),
        address: MemoryAddress,
        alignment: MemoryAlignment,
        length: u64,
    ) -> Result<(), Self::Error> {
        self.memory
            .validate(address, alignment, length)
            .map_err(MemoryFailure::from)
    }
    fn read_with_sideband(
        &self,
        (): &(),
        address: MemoryAddress,
        alignment: MemoryAlignment,
        bytes: &mut [u8],
    ) -> Result<(), Self::Error> {
        self.check(address)?;
        self.memory
            .read(address, alignment, bytes)
            .map_err(MemoryFailure::from)
    }
    fn write_with_sideband(
        &self,
        (): &(),
        address: MemoryAddress,
        alignment: MemoryAlignment,
        bytes: &[u8],
    ) -> Result<(), Self::Error> {
        self.check(address)?;
        self.memory
            .write(address, alignment, bytes)
            .map_err(MemoryFailure::from)
    }
    fn atomic64_with_sideband(
        &self,
        (): &(),
        address: MemoryAddress,
        operation: AtomicOperation<u64>,
    ) -> Result<u64, AtomicAccessError<Self::Error>> {
        self.check(address).map_err(AtomicAccessError::Access)?;
        let value = self
            .memory
            .atomic64(address, operation)
            .map_err(|error| match error {
                AtomicAccessError::Unsupported => AtomicAccessError::Unsupported,
                AtomicAccessError::Access(error) => {
                    AtomicAccessError::Access(MemoryFailure::from(error))
                }
            })?;
        self.commits.set(self.commits.get() + 1);
        Ok(value)
    }
}

impl FaultMemory {
    fn check(&self, address: MemoryAddress) -> Result<(), MemoryFailure> {
        if self.fault.get() && address.raw() == self.address {
            Err(MemoryFailure::PageFault { address })
        } else {
            Ok(())
        }
    }
}

fn rig(address: u64) -> Result<Rig<FaultMemory>, Box<dyn std::error::Error>> {
    rig_channels(address, 1)
}

fn rig_channels(
    address: u64,
    channels: usize,
) -> Result<Rig<FaultMemory>, Box<dyn std::error::Error>> {
    let initial = Rig::new(channels, 3)?;
    let memory = initial.memory;
    let backing = FaultMemory {
        memory: memory.clone(),
        fault: Rc::new(Cell::new(true)),
        address,
        commits: Rc::new(Cell::new(0)),
    };
    Rig::assemble(
        Dma::with_config(
            backing,
            DmaConfig {
                channels,
                capabilities: CapabilitiesValues::new(3),
            },
        )?,
        memory,
        channels,
    )
}

const fn add() -> Command {
    let operands = AtomicCommandOperandsValues::Add(AtomicCommandOperandsAddValues { value: 3 });
    Command::atomic(
        1,
        AtomicCommand {
            target: target(0x8000),
            order: AtomicCommandOrder::Relaxed,
            operation: operands.tag(),
            operands,
        },
    )
}

#[test]
fn atomic_fault_resumes_before_exactly_one_commit() -> TestResult {
    // Given: the atomic target faults before mutation.
    let mut rig = rig(0x8000)?;
    rig.submit(0, &[add(), fence(0x9000)])?;
    // When: software repairs and explicitly resumes the parked command.
    rig.drive()?;
    assert_eq!(rig.word(0x8000)?, 0);
    assert_eq!(rig.register(0, EVENT_PRODUCER)?, 0);
    assert_eq!(
        rig.register(
            0,
            ChannelBlock::CONTROL.offset()
                + ChannelRegisters::RECOVERY.offset()
                + ChannelRecovery::FAULT_STATUS.offset()
        )?,
        1
    );
    rig.dma.memory.fault.set(false);
    rig.dma.write(
        rig.bases.first().ok_or("missing channel")?
            + ChannelBlock::CONTROL.offset()
            + ChannelRegisters::RECOVERY.offset()
            + ChannelRecovery::RESUME.offset(),
        &1_u32.to_le_bytes(),
    )?;
    rig.drive()?;
    // Then: the original operation commits once and the successor fence completes.
    assert_eq!(rig.word(0x8000)?, 3);
    assert_eq!(rig.dma.memory.commits.get(), 1);
    assert_eq!(rig.word(0x9000)?, 0xfeed);
    Ok(())
}

#[test]
fn reset_after_atomic_effect_retires_without_repeating_or_undoing() -> TestResult {
    // Given: an atomic operation followed by a fence.
    let mut rig = rig(0xa000)?;
    rig.submit(0, &[add(), fence(0x9000)])?;
    // When: reset arrives immediately after the committed RMW.
    for _ in 0..256 {
        rig.step()?;
        if rig.dma.memory.commits.get() == 1 {
            break;
        }
    }
    assert_eq!(rig.dma.memory.commits.get(), 1);
    let reset = DmaDiscovery::EXTENT + DmaAdmin::CHANNELS_OFFSET;
    rig.dma.write(reset, &1_u32.to_le_bytes())?;
    rig.drive()?;
    // Then: retirement preserves the committed value and suppresses the later fence.
    assert_eq!(rig.word(0x8000)?, 3);
    assert_eq!(rig.dma.memory.commits.get(), 1);
    assert_eq!(rig.word(0x9000)?, 0);
    let mut bytes = [0; 4];
    rig.dma.read(reset, &mut bytes)?;
    assert_eq!(u32::from_le_bytes(bytes), 0);
    Ok(())
}

#[test]
fn fence_fault_resumes_publication_before_successors() -> TestResult {
    // Given: completion storage faults before the Fence can publish.
    let mut rig = rig(0x9000)?;
    rig.submit(
        0,
        &[
            fence(0x9000),
            Command::write(
                2,
                WordWrite {
                    target: target(0x8000),
                    value: 9,
                },
            ),
        ],
    )?;
    // When: the Fence write faults.
    rig.drive()?;
    // Then: the channel parks without an event or successor effects, then resumes in order.
    assert_eq!(rig.register(0, EVENT_PRODUCER)?, 0);
    assert_eq!(rig.register(0, CAUSE)? & 4, 0);
    assert_eq!(
        rig.register(
            0,
            ChannelBlock::CONTROL.offset()
                + ChannelRegisters::RECOVERY.offset()
                + ChannelRecovery::FAULT_STATUS.offset()
        )?,
        1
    );
    assert_eq!(rig.word(0x8000)?, 0);
    assert_eq!(rig.word(0x9000)?, 0);
    rig.dma.memory.fault.set(false);
    rig.dma.write(
        rig.bases.first().ok_or("missing channel")?
            + ChannelBlock::CONTROL.offset()
            + ChannelRegisters::RECOVERY.offset()
            + ChannelRecovery::RESUME.offset(),
        &1_u32.to_le_bytes(),
    )?;
    rig.drive()?;
    assert_eq!(rig.word(0x9000)?, 0xfeed);
    assert_eq!(rig.word(0x8000)?, 9);
    assert_eq!(rig.register(0, EVENT_PRODUCER)?, 0);
    Ok(())
}

#[test]
fn event_write_failure_keeps_stop_and_emits_fatal() -> TestResult {
    // Given: an invalid word command and faulting event storage.
    let mut rig = rig(0x1800)?;
    rig.submit(
        0,
        &[
            Command::write(
                1,
                WordWrite {
                    target: target(0x8001),
                    value: 1,
                },
            ),
            fence(0x9000),
        ],
    )?;
    // When: reporting the stopping error also fails.
    rig.drive()?;
    // Then: no event cursor advances, stop and fatal remain visible, and IRQ is independent.
    assert_eq!(rig.register(0, EVENT_PRODUCER)?, 0);
    assert_eq!(rig.register(0, CAUSE)? & 0b1100, 0b1100);
    assert!(rig.dma.take_interrupt());
    assert_eq!(rig.word(0x9000)?, 0);
    Ok(())
}
