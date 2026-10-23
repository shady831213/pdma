#![doc = "Copy-engine wire and notification boundary assertions."]

use pdma_model_contract::dma::{
    AtomicCommand, AtomicCommandOperandsCompareExchangeValues, AtomicCommandOperandsValues,
    AtomicCommandOperation, AtomicCommandOrder, Command, FillCommand, FillCommandWidth,
    SemaphoreCommand, SemaphoreCommandOperation, WaitCommand, WaitCommandComparison, WordTarget,
    WordTargetWidth, WordWrite,
};

#[test]
fn trap_decodes_when_wire_has_empty_payload() {
    // Given: an independently formed fixed-width Trap command.
    let mut wire = [0_u8; 112];
    wire[0] = 42;
    wire[8] = 7;
    // When: the wire crosses the typed command boundary.
    let decoded = Command::from_bytes(&wire);
    // Then: Trap is a valid command and preserves its identifier.
    assert_eq!(decoded.map(|command| command.id), Ok(42));
}

#[test]
fn command_families_match_independent_wire_layouts() {
    // Given: independently specified scalar command layouts and distinguishable fields.
    let target = WordTarget {
        address: 0x1238,
        context: 9,
        width: WordTargetWidth::Bits64,
    };
    let write = WordWrite {
        target,
        value: 0x5678,
    };
    let cases = [
        (
            Command::write(42, write),
            vec![
                (8, 1),
                (16, 0x38),
                (17, 0x12),
                (24, 9),
                (28, 1),
                (32, 0x78),
                (33, 0x56),
            ],
        ),
        (
            Command::fill(
                42,
                FillCommand {
                    address: 0x1238,
                    length: 19,
                    pattern: 0x5678,
                    context: 9,
                    width: FillCommandWidth::Bits32,
                },
            ),
            vec![
                (8, 2),
                (16, 0x38),
                (17, 0x12),
                (24, 19),
                (32, 0x78),
                (33, 0x56),
                (40, 9),
                (44, 1),
            ],
        ),
        (
            Command::wait(
                42,
                WaitCommand {
                    target,
                    value: 0x5678,
                    comparison: WaitCommandComparison::Reached,
                },
            ),
            vec![
                (8, 3),
                (16, 0x38),
                (17, 0x12),
                (24, 9),
                (28, 1),
                (32, 0x78),
                (33, 0x56),
                (40, 10),
            ],
        ),
        (
            Command::semaphore(
                42,
                SemaphoreCommand {
                    target,
                    operation: SemaphoreCommandOperation::Wait,
                },
            ),
            vec![(8, 5), (16, 0x38), (17, 0x12), (24, 9), (28, 1), (32, 1)],
        ),
        (
            Command::fence(42, write),
            vec![
                (8, 6),
                (16, 0x38),
                (17, 0x12),
                (24, 9),
                (28, 1),
                (32, 0x78),
                (33, 0x56),
            ],
        ),
        (Command::trap(42), vec![(8, 7)]),
    ];
    for (command, fields) in cases {
        assert_wire(command, &fields).unwrap();
    }
}

#[test]
fn malformed_nested_tags_are_rejected_at_the_wire_boundary() {
    // Given: wire records with one unknown discriminant at each nested enum boundary.
    let mutations = [
        (1, 28, 2),
        (2, 44, 3),
        (3, 40, 11),
        (4, 32, 4),
        (4, 36, 7),
        (5, 32, 2),
        (9, 8, 9),
    ];
    for (kind, offset, value) in mutations {
        let mut bytes = [0_u8; 112];
        bytes[8] = kind;
        *bytes.get_mut(offset).unwrap() = value;
        // When: hardware decodes the untrusted record.
        let decoded = Command::from_bytes(&bytes);
        // Then: an unknown operation cannot enter typed execution.
        assert!(decoded.is_err(), "kind {kind}, byte {offset}");
    }
}

#[test]
fn compare_exchange_preserves_two_operands_without_result_storage() {
    let target = WordTarget {
        address: 0x1238,
        context: 9,
        width: WordTargetWidth::Bits64,
    };
    let (command, fields) = (
        Command::atomic(
            42,
            AtomicCommand {
                target,
                order: AtomicCommandOrder::AcquireRelease,
                operation: AtomicCommandOperation::CompareExchange,
                operands: AtomicCommandOperandsValues::CompareExchange(
                    AtomicCommandOperandsCompareExchangeValues {
                        compare: 0x5678,
                        value: 0x9abc,
                    },
                ),
            },
        ),
        vec![
            (8, 4),
            (16, 0x38),
            (17, 0x12),
            (24, 9),
            (28, 1),
            (32, 3),
            (36, 6),
            (40, 0x78),
            (41, 0x56),
            (48, 0xbc),
            (49, 0x9a),
        ],
    );
    assert_wire(command, &fields).unwrap();
}

fn assert_wire(command: Command, fields: &[(usize, u8)]) -> Result<(), Box<dyn std::error::Error>> {
    let mut expected = [0_u8; 112];
    expected[0] = 42;
    for &(offset, value) in fields {
        *expected.get_mut(offset).ok_or("fixture byte offset")? = value;
    }
    // When: a command is encoded and the independent wire is decoded.
    let encoded = command.to_bytes()?;
    let decoded = Command::from_bytes(&expected)?;
    // Then: every byte, including padding, matches and the payload survives.
    assert_eq!(encoded, expected);
    assert_eq!(decoded, command);
    Ok(())
}
