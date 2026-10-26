//! Complete command enums compose through the same codec boundary as records.
use hardware_interface_schema::{SchemaCodecError, record_schema, union_schema};
use pdma_model_contract::dma::{Command, CommandHeader, Opcode, TrapCommand};

record_schema! {
    pub record Stamped {
        endian: Little,
        fields: { command: Command, stamp: u64 },
    }
}

union_schema! {
    pub union Envelope {
        cases: { Command(Command), Stamped(Stamped) },
    }
}

const fn command() -> Command {
    Command::Trap(TrapCommand {
        header: CommandHeader {
            opcode: Opcode::Trap,
        },
        id: 0x1234_5678_9abc_def0,
    })
}

#[test]
fn command_enum_round_trips_as_an_untagged_branch() {
    // Given: a nonzero complete command inside a larger union slot.
    let value = Envelope::Command(command());
    let mut bytes = [0xff; Envelope::RECORD_SIZE];
    // When: encoding and explicitly decoding the outer branch.
    value.encode_variant(&mut bytes).unwrap();
    let decoded = Envelope::decode_variant(&bytes, EnvelopeCase::Command).unwrap();
    // Then: the inner opaque storage is payload, not padding.
    assert_eq!(decoded, value);
    assert_eq!(
        &bytes[..Command::RECORD_SIZE],
        &command().to_bytes().unwrap()
    );
    assert_eq!(&bytes[Command::RECORD_SIZE..], &[0; 8]);
}

#[test]
fn command_enum_round_trips_as_a_record_field_and_view() {
    // Given: a record containing the complete command schema enum.
    let value = Stamped::new(command(), 0xfedc_ba98_7654_3210);
    // When: the generated record codec and nested view traverse the enum field.
    let bytes = value.to_bytes().unwrap();
    let decoded = Stamped::from_bytes(&bytes).unwrap();
    let nested = StampedView::new(&bytes)
        .unwrap()
        .command()
        .unwrap()
        .values()
        .unwrap();
    // Then: both preserve protocol decoding and unrelated sibling bytes.
    assert_eq!(decoded, value);
    assert_eq!(nested, command());
    assert_eq!(&bytes[Command::RECORD_SIZE..], &value.stamp.to_le_bytes());
}

#[test]
fn each_layer_rejects_its_own_reserved_storage() {
    // Given: one valid nested command slot.
    let mut bytes = [0; Envelope::RECORD_SIZE];
    Envelope::Command(command())
        .encode_variant(&mut bytes)
        .unwrap();
    let mut inner_gap = bytes;
    inner_gap[1] = 1;
    let mut outer_tail = bytes;
    outer_tail[Command::RECORD_SIZE] = 1;
    // When: invalid inner padding and invalid outer tail are decoded separately.
    let inner_error = Envelope::decode_variant(&inner_gap, EnvelopeCase::Command);
    let outer_error = Envelope::decode_variant(&outer_tail, EnvelopeCase::Command);
    // Then: neither opaque nesting nor trait delegation weakens reserved-byte checks.
    assert!(matches!(
        inner_error,
        Err(SchemaCodecError::NonZeroReserved { .. })
    ));
    assert!(matches!(
        outer_error,
        Err(SchemaCodecError::NonZeroReserved { location, .. })
            if location.identity().owner() == "Envelope"
                && location.identity().member() == "tail"
    ));
}
