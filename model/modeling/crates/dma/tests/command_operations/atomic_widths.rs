use super::support::{EVENT_PRODUCER, Rig, TestResult, fence, target};
use hw_model_io::memory::{MemoryAccess, MemoryAddress, MemoryAlignment};
use pdma_model_contract::dma::{
    AtomicCommand, AtomicCommandOperandsAddValues, AtomicCommandOperandsAndValues,
    AtomicCommandOperandsCompareExchangeValues, AtomicCommandOperandsExchangeValues,
    AtomicCommandOperandsOrValues, AtomicCommandOperandsSubtractValues,
    AtomicCommandOperandsValues as Operands, AtomicCommandOperandsXorValues, AtomicCommandOrder,
    Command, CommandHeader, Opcode, WordTargetWidth,
};

#[test]
fn each_atomic_operation_uses_width_truncated_bits() -> TestResult {
    // Given: one input value and independent expected results for every RMW kind.
    let cases = [
        (
            Operands::Add(AtomicCommandOperandsAddValues { value: u64::MAX }),
            9,
        ),
        (
            Operands::Subtract(AtomicCommandOperandsSubtractValues { value: 3 }),
            7,
        ),
        (
            Operands::And(AtomicCommandOperandsAndValues { value: 3 }),
            2,
        ),
        (Operands::Or(AtomicCommandOperandsOrValues { value: 5 }), 15),
        (
            Operands::Xor(AtomicCommandOperandsXorValues { value: 3 }),
            9,
        ),
        (
            Operands::Exchange(AtomicCommandOperandsExchangeValues { value: 7 }),
            7,
        ),
        (
            Operands::CompareExchange(AtomicCommandOperandsCompareExchangeValues {
                compare: 10,
                value: 13,
            }),
            13,
        ),
    ];
    for width in [WordTargetWidth::Bits32, WordTargetWidth::Bits64] {
        for (operands, expected) in cases {
            let mut rig = Rig::new(1, 3)?;
            let initial = match width {
                WordTargetWidth::Bits32 => 0x1122_3344_0000_000a_u64,
                WordTargetWidth::Bits64 => 10,
            };
            rig.memory.write(
                MemoryAddress::new(0x8000),
                MemoryAlignment::ONE,
                &initial.to_le_bytes(),
            )?;
            let mut target = target(0x8000);
            target.width = width;
            rig.submit(
                0,
                &[
                    Command::Atomic(AtomicCommand {
                        header: CommandHeader {
                            opcode: Opcode::Atomic,
                        },
                        id: 1,
                        target,
                        order: AtomicCommandOrder::Release,
                        operation: operands.tag(),
                        operands,
                    }),
                    fence(0x9000),
                ],
            )?;
            // When: hardware performs the selected fixed-width update.
            rig.drive()?;
            // Then: the arithmetic result is exact and adjacent high bytes stay intact.
            let expected = match width {
                WordTargetWidth::Bits32 => 0x1122_3344_0000_0000 | expected,
                WordTargetWidth::Bits64 => expected,
            };
            assert_eq!(rig.word(0x8000)?, expected);
            assert_eq!(rig.word(0x9000)?, 0xfeed);
            assert_eq!(rig.register(0, EVENT_PRODUCER)?, 0);
        }
    }
    Ok(())
}
