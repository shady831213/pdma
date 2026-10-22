//! Host-owned ring configuration and constructor validation.

use pdma_model_contract::dma::{Command, Event};
use pdma_model_contract::ring::{Ring, RingError};

#[test]
fn configuration_uses_the_selected_entry_layout() -> Result<(), Box<dyn std::error::Error>> {
    // Given
    let command_base = 0x1000;
    let event_base = 0x2000;
    let command = pdma_model_host::RingConfig::<Command>::new(command_base, 2)?;
    let event = pdma_model_host::RingConfig::<Event>::new(event_base, 2)?;
    // When
    let addresses = (command.entry(1)?, event.entry(1)?);
    // Then
    assert_eq!(
        addresses,
        (
            command_base + u64::try_from(Command::RECORD_SIZE)?,
            event_base + u64::try_from(Event::RECORD_SIZE)?,
        )
    );
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
