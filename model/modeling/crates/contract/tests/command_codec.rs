//! Protocol-aware command views must use the contract's checks.
use hardware_interface_schema::{Decode, Encode, RecordLayout, SchemaCodecError};
use pdma_model_contract::dma::{
    Command, CommandHeader, CommandView, CommandViewMut, Opcode, TrapCommand,
};

#[test]
fn normal_view_rejects_inconsistent_opcode_before_mutation() {
    // Given: valid wire storage and an inconsistent semantic command.
    let valid = Command::Trap(TrapCommand {
        header: CommandHeader {
            opcode: Opcode::Trap,
        },
        id: 42,
    });
    let mut bytes = valid.to_bytes().unwrap();
    let original = bytes;
    let invalid = Command::Trap(TrapCommand {
        header: CommandHeader {
            opcode: Opcode::Copy,
        },
        id: 99,
    });
    // When: replacing through the normal command view.
    let result = CommandViewMut::new(&mut bytes)
        .unwrap()
        .set_values(&invalid);
    // Then: protocol checks run before touching storage.
    assert!(matches!(
        result,
        Err(SchemaCodecError::ConstantMismatch { .. })
    ));
    assert_eq!(bytes, original);
}

#[test]
fn standard_helpers_use_the_external_command_codec() {
    // Given: independent wire bytes for one complete Trap command.
    let mut wire = [0_u8; 96];
    wire[0] = 7;
    wire[8..16].copy_from_slice(&u64::MAX.to_le_bytes());
    let expected = Command::Trap(TrapCommand {
        header: CommandHeader {
            opcode: Opcode::Trap,
        },
        id: u64::MAX,
    });
    // When: the bytes pass through generated adapters and normal views.
    let value = <Command as RecordLayout>::decode(&wire).unwrap();
    let view = CommandView::new(&wire).unwrap().decode().unwrap();
    let mut destination = [0xa5; 96];
    let mut writer = <Command as RecordLayout>::view_mut(&mut destination).unwrap();
    writer.set_values(&value).unwrap();
    // Then: each path preserves external protocol interpretation and exact bytes.
    assert_eq!(value, expected);
    assert_eq!(view, expected);
    assert_eq!(writer.values().unwrap(), expected);
    assert_eq!(destination, wire);
    assert_eq!(expected.encode().unwrap().decode().unwrap(), expected);
}
