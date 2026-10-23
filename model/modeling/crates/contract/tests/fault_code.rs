//! Typed channel fatal codes over the unchanged MMIO representation.

use hardware_interface_schema::RegisterValueError;
use pdma_model_contract::dma::{
    ChannelFatal, ChannelFatalCode, ChannelFatalValues, ChannelRegistersValues,
};

#[test]
fn reset_fault_decodes_to_none() -> Result<(), Box<dyn std::error::Error>> {
    // Given: reset channel registers.
    let registers = ChannelRegistersValues::default();
    // When: software interprets the fatal field.
    let code = registers.recovery.fatal.code()?;
    // Then: the reset encoding denotes no fatal.
    assert_eq!(code, ChannelFatalCode::None);
    assert_eq!(registers.recovery.fatal.raw(), 0);
    Ok(())
}

#[test]
fn typed_faults_preserve_existing_wire_codes() -> Result<(), Box<dyn std::error::Error>> {
    // Given: the three defined hardware states and their wire encodings.
    for (code, raw) in [
        (ChannelFatalCode::None, 0),
        (ChannelFatalCode::CommandRead, 1),
        (ChannelFatalCode::EventWrite, 2),
        (ChannelFatalCode::EventFull, 3),
        (ChannelFatalCode::EventConfiguration, 4),
    ] {
        // When: hardware writes an enum into the register field.
        let register = ChannelFatalValues::default().with_code(code);
        // Then: typed and raw readers observe the same state.
        assert_eq!(register.raw(), raw);
        assert_eq!(register.code()?, code);
        assert_eq!(ChannelFatalCode::from_raw(raw)?, code);
    }
    Ok(())
}

#[test]
fn unknown_fault_encoding_stays_raw_and_rejects_typed_decode() {
    // Given: a bus value outside the declared enum.
    let register = ChannelFatalValues::new(5);
    // When: interpreting its code as a hardware state.
    let decoded = register.code();
    // Then: unknown is an error, never the no-fatal state; raw inspection remains possible.
    assert_eq!(
        decoded,
        Err(RegisterValueError::UnknownTag {
            location: ChannelFatal::CODE_IDENTITY.at_bit(0),
            observed: 5,
        })
    );
    assert_eq!(register.raw(), 5);
}
