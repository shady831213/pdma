use super::{TestResult, add, fence, rig, target};
use crate::command_operations::support::{CAUSE, EVENT_PRODUCER};
use hardware_interface_runtime::{MemoryAccess, MemoryAddress, MemoryAlignment};
use pdma_model_contract::dma::{
    ChannelBlock, ChannelRegisters, Command, CommandRegisters, DmaAdmin, DmaDiscovery, FillCommand,
    FillCommandWidth, SemaphoreCommand, SemaphoreCommandOperation, WaitCommand,
    WaitCommandComparison, WordWrite,
};

#[test]
fn scalar_faults_retain_commands_until_resume() -> TestResult {
    // Given: each scalar family encounters a recoverable target fault.
    let commands = [
        Command::write(
            1,
            WordWrite {
                target: target(0x8000),
                value: 42,
            },
        ),
        Command::wait(
            2,
            WaitCommand {
                target: target(0x8000),
                value: 0,
                comparison: WaitCommandComparison::Equal,
            },
        ),
        Command::semaphore(
            3,
            SemaphoreCommand {
                target: target(0x8000),
                operation: SemaphoreCommandOperation::Signal,
            },
        ),
    ];
    for (command, expected) in commands.into_iter().zip([42, 0, 1]) {
        let mut rig = rig(0x8000)?;
        rig.submit(0, &[command, fence(0x9000)])?;
        // When: execution faults, then software repairs and resumes it.
        rig.drive()?;
        assert_eq!(rig.word(0x9000)?, 0);
        assert_eq!(rig.register(0, EVENT_PRODUCER)?, 0);
        assert_eq!(rig.register(0, CAUSE)?, 0);
        assert_eq!(
            rig.register(
                0,
                ChannelBlock::CONTROL.offset() + ChannelRegisters::FAULT_STATUS.offset()
            )?,
            1
        );
        rig.dma.memory.fault.set(false);
        rig.dma.write(
            rig.bases.first().ok_or("missing channel")?
                + ChannelBlock::CONTROL.offset()
                + ChannelRegisters::RESUME.offset(),
            &1_u32.to_le_bytes(),
        )?;
        rig.drive()?;
        // Then: the original command resumes successfully before its Fence.
        assert_eq!(rig.word(0x8000)?, expected);
        assert_eq!(rig.word(0x9000)?, 0xfeed);
        assert_eq!(rig.register(0, EVENT_PRODUCER)?, 0);
    }
    Ok(())
}

#[test]
fn fill_fault_retains_committed_prefix() -> TestResult {
    // Given: a fill faults at its second chunk after committing the first.
    let mut rig = rig(0x8100)?;
    rig.submit(
        0,
        &[
            Command::fill(
                1,
                FillCommand {
                    address: 0x8000,
                    length: 259,
                    pattern: 0x0403_0201,
                    context: 0,
                    width: FillCommandWidth::Bits32,
                },
            ),
            fence(0x9000),
        ],
    )?;
    // When: software changes the committed prefix, repairs, and resumes.
    rig.drive()?;
    assert_eq!(rig.word(0x8000)?, 0x0403_0201_0403_0201);
    rig.memory
        .write(MemoryAddress::new(0x8000), MemoryAlignment::ONE, &[9; 8])?;
    rig.dma.memory.fault.set(false);
    rig.dma.write(
        rig.bases.first().ok_or("missing channel")?
            + ChannelBlock::CONTROL.offset()
            + ChannelRegisters::RESUME.offset(),
        &1_u32.to_le_bytes(),
    )?;
    rig.drive()?;
    // Then: the completed prefix is not replayed and the exact partial tail commits.
    assert_eq!(rig.word(0x8000)?, 0x0909_0909_0909_0909);
    assert_eq!(rig.word(0x8100)?, 0x0003_0201);
    assert_eq!(rig.word(0x9000)?, 0xfeed);
    Ok(())
}

#[test]
fn reset_before_atomic_effect_prevents_mutation() -> TestResult {
    // Given: a fetched atomic command whose target operation has not run.
    let mut rig = rig(0xa000)?;
    rig.submit(0, &[add(), fence(0x9000)])?;
    let consumer = ChannelBlock::COMMAND.offset() + CommandRegisters::CONSUMER.offset();
    for _ in 0..256 {
        rig.step()?;
        if rig.register(0, consumer)? == 1 {
            break;
        }
    }
    assert_eq!(rig.register(0, consumer)?, 1);
    assert_eq!(rig.dma.memory.commits.get(), 0);
    // When: reset invalidates the resident command before execution.
    rig.dma.write(
        DmaDiscovery::EXTENT + DmaAdmin::CHANNELS_OFFSET,
        &1_u32.to_le_bytes(),
    )?;
    rig.drive()?;
    // Then: neither the atomic effect nor its Fence can occur.
    assert_eq!(rig.word(0x8000)?, 0);
    assert_eq!(rig.dma.memory.commits.get(), 0);
    assert_eq!(rig.word(0x9000)?, 0);
    Ok(())
}

#[test]
fn backend_unsupported_rejects_advertised_atomic_without_effects() -> TestResult {
    // Given: the instance advertises 32-bit atomics but its injected port rejects them.
    let mut rig = rig(0xa000)?;
    let operands = pdma_model_contract::dma::AtomicCommandOperandsValues::Add(
        pdma_model_contract::dma::AtomicCommandOperandsAddValues { value: 7 },
    );
    let command = Command::atomic(
        1,
        pdma_model_contract::dma::AtomicCommand {
            target: pdma_model_contract::dma::WordTarget {
                width: pdma_model_contract::dma::WordTargetWidth::Bits32,
                ..target(0x8000)
            },
            order: pdma_model_contract::dma::AtomicCommandOrder::Relaxed,
            operation: operands.tag(),
            operands,
        },
    );
    rig.submit(0, &[command, fence(0x9000)])?;
    // When: the backend reports Unsupported before mutation.
    rig.drive()?;
    // Then: the stopping event preserves Unsupported and prevents the Fence.
    assert_eq!(rig.word(0x8000)?, 0);
    assert_eq!(rig.word(0x9000)?, 0);
    assert_eq!(rig.register(0, EVENT_PRODUCER)?, 1);
    let mut bytes = vec![0; pdma_model_contract::dma::Event::RECORD_SIZE];
    rig.memory
        .read(MemoryAddress::new(0x1800), MemoryAlignment::ONE, &mut bytes)?;
    assert_eq!(
        pdma_model_contract::dma::Event::from_bytes(&bytes)?.code,
        pdma_model_contract::dma::EventCode::Unsupported
    );
    Ok(())
}
