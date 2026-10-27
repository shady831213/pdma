use super::support::{CAUSE, EVENT_PRODUCER, Rig, TestResult, fence, target};
use hardware_interface_io::memory::{MemoryAccess, MemoryAddress, MemoryAlignment};
use pdma_model_contract::dma::{
    ChannelBlock, ChannelInterrupts, ChannelRegisters, Command, CommandHeader, Opcode, TrapCommand,
    WaitCommand, WaitCommandComparison,
};

#[test]
fn unmask_does_not_replay_trap_but_a_later_trap_emits() -> TestResult {
    // Given: a masked Trap, an ordinary dependency, and a later Trap.
    let mut rig = Rig::new(1, 0)?;
    let mask = rig.bases.first().ok_or("missing channel")?
        + ChannelBlock::CONTROL.offset()
        + ChannelRegisters::IRQ.offset()
        + ChannelInterrupts::MASK.offset();
    rig.dma.write(mask, &2_u32.to_le_bytes())?;
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
            Command::Wait(WaitCommand {
                header: CommandHeader {
                    opcode: Opcode::Wait,
                },
                id: 2,
                target: target(0x8000),
                value: 1,
                comparison: WaitCommandComparison::Equal,
            }),
            Command::Trap(TrapCommand {
                header: CommandHeader {
                    opcode: Opcode::Trap,
                },
                id: 3,
            }),
            fence(0x9010),
        ],
    )?;
    rig.drive()?;
    assert_eq!(rig.word(0x9000)?, 0xfeed);
    assert_eq!(rig.register(0, CAUSE)? & 2, 2);
    assert!(!rig.dma.take_interrupt());
    // When: software unmasks the source, then permits the later Trap.
    rig.dma.write(mask, &0_u32.to_le_bytes())?;
    rig.drive()?;
    assert!(!rig.dma.take_interrupt());
    rig.memory.write(
        MemoryAddress::new(0x8000),
        MemoryAlignment::ONE,
        &1_u64.to_le_bytes(),
    )?;
    rig.drive()?;
    // Then: the new Trap emits even while its earlier sticky cause is still set.
    assert!(rig.dma.take_interrupt());
    assert_eq!(rig.word(0x9010)?, 0xfeed);
    assert_eq!(rig.register(0, EVENT_PRODUCER)?, 0);
    Ok(())
}
