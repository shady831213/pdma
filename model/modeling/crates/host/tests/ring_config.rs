//! Host-owned ring configuration and constructor validation.

use pdma_model_contract::{Command, Response, Ring, RingError};

#[test]
fn configuration_uses_the_selected_entry_layout() -> Result<(), RingError> {
    // Given
    let command = pdma_model_host::RingConfig::<Command>::new(0x1000, 2)?;
    let response = pdma_model_host::RingConfig::<Response>::new(0x2000, 2)?;
    // When
    let addresses = (command.entry(1)?, response.entry(1)?);
    // Then
    assert_eq!(addresses, (0x1020, 0x2010));
    Ok(())
}

#[test]
fn configuration_rejects_invalid_storage_before_setup() {
    for (base, capacity) in [
        (0x1000, 0),
        (0x1000, 3),
        (0x1001, 2),
        (0x1000, 1 << 31),
        (u64::MAX - 7, 2),
    ] {
        // Given: software-selected storage that cannot form a legal ring.
        // When
        let result = pdma_model_host::RingConfig::<Command>::new(base, capacity);
        // Then
        assert!(matches!(result, Err(RingError::Geometry)));
    }
}
