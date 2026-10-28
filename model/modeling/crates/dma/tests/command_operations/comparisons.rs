use super::support::{Rig, TestResult, fence, target};
use hw_model_io::memory::{MemoryAccess, MemoryAddress, MemoryAlignment};
use pdma_model_contract::dma::{
    Command, CommandHeader, Opcode, WaitCommand, WaitCommandComparison as Compare, WordTargetWidth,
};

#[test]
fn wait_comparisons_use_selected_width_and_signed_order() -> TestResult {
    // Given: unsigned, signed, and modular comparisons with boundary values.
    let cases = [
        (Compare::Equal, 3, 3, true),
        (Compare::NotEqual, 3, 4, true),
        (Compare::UnsignedLess, 3, 4, true),
        (Compare::UnsignedLessEqual, 3, 3, true),
        (Compare::UnsignedGreater, 4, 3, true),
        (Compare::UnsignedGreaterEqual, 3, 3, true),
        (Compare::SignedLess, u64::MAX, 0, true),
        (Compare::SignedLessEqual, u64::MAX, u64::MAX, true),
        (Compare::SignedGreater, 0, u64::MAX, true),
        (Compare::SignedGreaterEqual, u64::MAX, u64::MAX, true),
        (Compare::Reached, 1, u64::MAX, true),
        (Compare::Reached, u64::MAX, 1, false),
        (Compare::UnsignedLess, u64::MAX, 0, false),
        (Compare::Equal, 3, 4, false),
    ];
    for width in [WordTargetWidth::Bits32, WordTargetWidth::Bits64] {
        for (comparison, observed, value, expected) in cases {
            let mut rig = Rig::new(1, 0)?;
            rig.memory.write(
                MemoryAddress::new(0x8000),
                MemoryAlignment::ONE,
                &observed.to_le_bytes(),
            )?;
            let mut target = target(0x8000);
            target.width = width;
            rig.submit(
                0,
                &[
                    Command::Wait(WaitCommand {
                        header: CommandHeader {
                            opcode: Opcode::Wait,
                        },
                        id: 1,
                        target,
                        value,
                        comparison,
                    }),
                    fence(0x9000),
                ],
            )?;
            // When: the dependency is evaluated through hardware.
            rig.drive()?;
            // Then: only a satisfied comparison permits the following Fence.
            assert_eq!(
                rig.word(0x9000)? == 0xfeed,
                expected,
                "{width:?} {comparison:?}"
            );
        }
    }
    Ok(())
}
