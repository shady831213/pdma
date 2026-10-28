//! Black-box register declaration tests.

use hw_model_schema::{
    Endian,
    register::{
        Access, RegisterDefinition, RegisterDefinitionError, RegisterSpec, RegisterValidationError,
        ResetKind,
    },
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RegisterId {
    Byte,
    Half,
    Word,
    Double,
}

const fn register(id: RegisterId, name: &'static str, width: usize) -> RegisterSpec<RegisterId> {
    RegisterSpec::new(id, name, 0, width)
}

#[test]
fn register_spec_preserves_supported_widths_and_all_metadata() {
    // Given
    const REGISTERS: [RegisterSpec<RegisterId>; 4] = [
        register(RegisterId::Byte, "byte", 1),
        register(RegisterId::Half, "half", 2)
            .with_access(Access::WriteOnly)
            .with_reset(ResetKind::PostReset, 0x1234)
            .with_writable_mask(0x00ff),
        register(RegisterId::Word, "word", 4)
            .with_access(Access::ReadWrite)
            .with_reset(ResetKind::Constant, 0x89ab_cdef)
            .with_writable_mask(0xffff_0000),
        register(RegisterId::Double, "double", 8)
            .with_access(Access::ReadOnly)
            .with_reset(ResetKind::Zero, u64::MAX)
            .with_writable_mask(u64::MAX),
    ];

    // When
    let results = REGISTERS.map(RegisterSpec::validate);

    // Then
    assert_eq!(results, [Ok(()); 4]);
    assert_eq!(REGISTERS[0].width(), 1);
    assert_eq!(REGISTERS[1].access(), Access::WriteOnly);
    assert_eq!(REGISTERS[1].reset_kind(), ResetKind::PostReset);
    assert_eq!(REGISTERS[1].reset_value(), 0x1234);
    assert_eq!(REGISTERS[1].writable_mask(), 0x00ff);
    assert_eq!(REGISTERS[2].id(), RegisterId::Word);
    assert_eq!(REGISTERS[2].name(), "word");
    assert_eq!(REGISTERS[2].offset(), 0);
    assert_eq!(REGISTERS[3].width(), 8);
}

#[test]
fn register_validation_rejects_invalid_width() {
    // Given
    let malformed = register(RegisterId::Byte, "odd", 3);

    // When
    let result = malformed.validate();

    // Then
    assert_eq!(
        result,
        Err(RegisterValidationError::InvalidWidth {
            name: "odd",
            width: 3,
        })
    );
}

#[test]
fn register_definition_validation_rejects_invalid_alignment() {
    let definition =
        RegisterDefinition::<u16>::new("misaligned", 4, Endian::Little).with_alignment(3);

    assert!(matches!(
        definition.validate(),
        Err(RegisterDefinitionError::InvalidAlignment { alignment: 3, .. })
    ));
}

#[test]
fn register_validation_rejects_writable_mask_overflow() {
    // Given
    let malformed = register(RegisterId::Byte, "mask", 1).with_writable_mask(0x100);

    // When
    let result = malformed.validate();

    // Then
    assert_eq!(
        result,
        Err(RegisterValidationError::WritableMaskOverflow {
            name: "mask",
            width: 1,
            mask: 0x100,
        })
    );
}

#[test]
fn register_validation_rejects_reset_value_overflow() {
    // Given
    let malformed = register(RegisterId::Byte, "reset", 1).with_reset(ResetKind::Constant, 0x100);

    // When
    let result = malformed.validate();

    // Then
    assert_eq!(
        result,
        Err(RegisterValidationError::ResetValueOverflow {
            name: "reset",
            width: 1,
            value: 0x100,
        })
    );
}

#[test]
fn register_validation_prioritizes_width_then_mask_then_reset() {
    // Given
    let bad_width = register(RegisterId::Byte, "mixed", 3)
        .with_writable_mask(u64::MAX)
        .with_reset(ResetKind::Constant, u64::MAX);
    let bad_mask = register(RegisterId::Byte, "mixed", 1)
        .with_writable_mask(0x100)
        .with_reset(ResetKind::Constant, 0x100);

    // When
    let results = [bad_width.validate(), bad_mask.validate()];

    // Then
    assert_eq!(
        results,
        [
            Err(RegisterValidationError::InvalidWidth {
                name: "mixed",
                width: 3,
            }),
            Err(RegisterValidationError::WritableMaskOverflow {
                name: "mixed",
                width: 1,
                mask: 0x100,
            }),
        ]
    );
}
