use super::support::{CAUSE, EVENT_PRODUCER, Rig, TestResult, fence, target};
use hardware_interface_runtime::{MemoryAccess, MemoryAddress, MemoryAlignment};
use pdma_model_contract::dma::{
    ChannelBlock, ChannelInterrupts, ChannelRecovery, ChannelRegisters, Command, CommandHeader,
    FillCommand, FillCommandWidth, Opcode, TrapCommand, WaitCommand, WaitCommandComparison,
    WordWrite, WriteCommand,
};

#[test]
fn write_fill_and_fence_commit_without_events() -> TestResult {
    // Given: an unaligned pattern destination with a partial tail.
    let mut rig = Rig::new(1, 0)?;
    rig.submit(
        0,
        &[
            Command::Write(WriteCommand {
                header: CommandHeader {
                    opcode: Opcode::Write,
                },
                id: 1,
                word: WordWrite {
                    target: target(0x8000),
                    value: 0x1234,
                },
            }),
            Command::Fill(FillCommand {
                header: CommandHeader {
                    opcode: Opcode::Fill,
                },
                id: 2,
                address: 0x8101,
                length: 259,
                pattern: 0x0403_0201,
                context: 0,
                width: FillCommandWidth::Bits32,
            }),
            fence(0x9000),
        ],
    )?;
    // When: all commands execute on the shared hardware pipeline.
    rig.drive()?;
    // Then: the fence follows complete stores without a success event.
    assert_eq!(rig.word(0x9000)?, 0xfeed);
    assert_eq!(rig.word(0x8000)?, 0x1234);
    let mut bytes = [0; 259];
    rig.memory
        .read(MemoryAddress::new(0x8101), MemoryAlignment::ONE, &mut bytes)?;
    assert!(
        bytes
            .iter()
            .enumerate()
            .all(|(index, byte)| usize::from(*byte) == index % 4 + 1)
    );
    assert_eq!(rig.register(0, EVENT_PRODUCER)?, 0);
    Ok(())
}

#[test]
fn false_wait_yields_to_peer_and_blocks_own_fence() -> TestResult {
    // Given: channel zero waits for a value channel one will publish.
    let mut rig = Rig::new(2, 0)?;
    rig.submit(
        0,
        &[
            Command::Wait(WaitCommand {
                header: CommandHeader {
                    opcode: Opcode::Wait,
                },
                id: 1,
                target: target(0x8000),
                value: 7,
                comparison: WaitCommandComparison::Equal,
            }),
            fence(0x9000),
        ],
    )?;
    // When: the wait is false, then a peer publishes the prerequisite.
    rig.drive()?;
    assert_eq!(rig.word(0x9000)?, 0);
    assert_eq!(
        rig.register(
            0,
            ChannelBlock::CONTROL.offset()
                + ChannelRegisters::RECOVERY.offset()
                + ChannelRecovery::FAULT_STATUS.offset()
        )?,
        0
    );
    rig.submit(
        1,
        &[
            Command::Write(WriteCommand {
                header: CommandHeader {
                    opcode: Opcode::Write,
                },
                id: 2,
                word: WordWrite {
                    target: target(0x8000),
                    value: 7,
                },
            }),
            fence(0x9010),
        ],
    )?;
    rig.drive()?;
    // Then: both channels complete without software RESUME or events.
    assert_eq!(rig.word(0x9000)?, 0xfeed);
    assert_eq!(rig.word(0x9010)?, 0xfeed);
    assert_eq!(rig.register(0, EVENT_PRODUCER)?, 0);
    Ok(())
}

#[test]
fn trap_ack_preserves_pending_notification() -> TestResult {
    // Given: a Trap followed by software-visible completion storage.
    let mut rig = Rig::new(1, 0)?;
    rig.submit(
        0,
        &[
            Command::Trap(TrapCommand {
                header: CommandHeader {
                    opcode: Opcode::Trap,
                },
                id: 1,
            }),
            fence(0x9000),
        ],
    )?;
    // When: software acknowledges the sticky cause before claiming the IRQ.
    rig.drive()?;
    assert_eq!(rig.register(0, CAUSE)? & 2, 2);
    rig.dma.write(
        rig.bases.first().ok_or("missing channel")?
            + ChannelBlock::CONTROL.offset()
            + ChannelRegisters::IRQ.offset()
            + ChannelInterrupts::TRAP_ACK.offset(),
        &1_u32.to_le_bytes(),
    )?;
    // Then: acknowledgement clears the cause but leaves the emitted notification.
    assert_eq!(rig.register(0, CAUSE)? & 2, 0);
    assert!(rig.dma.take_interrupt());
    assert_eq!(rig.register(0, EVENT_PRODUCER)?, 0);
    Ok(())
}

#[test]
fn abort_cancels_wait_without_fault_resume() -> TestResult {
    // Given: an ordinary retained Wait referencing a payload-only context.
    let mut rig = Rig::new(1, 0)?;
    rig.submit(
        0,
        &[Command::Wait(WaitCommand {
            header: CommandHeader {
                opcode: Opcode::Wait,
            },
            id: 1,
            target: pdma_model_contract::dma::WordTarget {
                context: 1,
                ..target(0x8000)
            },
            value: 1,
            comparison: WaitCommandComparison::Equal,
        })],
    )?;
    rig.drive()?;
    // When: context Abort cancels that wait.
    let context = pdma_model_contract::dma::ContextId::new(1);
    rig.dma
        .abort_context(context, &[pdma_model_contract::dma::ChannelId::new(0)])?;
    let mut done = rig.dma.context_status(context)?;
    for _ in 0..256 {
        if done {
            break;
        }
        rig.step()?;
        done = rig.dma.context_status(context)?;
    }
    // Then: maintenance completes without a fault, event, or target write.
    assert!(done);
    assert_eq!(rig.word(0x8000)?, 0);
    assert_eq!(rig.register(0, EVENT_PRODUCER)?, 0);
    assert_eq!(
        rig.register(
            0,
            ChannelBlock::CONTROL.offset()
                + ChannelRegisters::RECOVERY.offset()
                + ChannelRecovery::FAULT_STATUS.offset()
        )?,
        0
    );
    Ok(())
}
