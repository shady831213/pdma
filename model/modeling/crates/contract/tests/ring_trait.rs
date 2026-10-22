//! Downstream rings supply their own state and reuse the contract algorithms.

use pdma_model_contract::dma::{Command, CommandRegistersValues};
use pdma_model_contract::ring::{Ring, RingError, RingPointers};

struct RegisterRing {
    registers: CommandRegistersValues,
}

impl Ring for RegisterRing {
    type Entry = Command;

    fn base(&self) -> u64 {
        self.registers.base.raw()
    }

    fn capacity(&self) -> u32 {
        self.registers.capacity.raw()
    }
}

#[test]
fn entry_stride_is_available_before_ring_configuration() -> Result<(), RingError> {
    // Given: the record layout exists even while the ring is unconfigured.
    let ring = RegisterRing {
        registers: CommandRegistersValues::default(),
    };
    // When
    let stride = ring.entry_stride()?;
    // Then
    assert_eq!(stride, 112);
    assert_eq!(ring.validate_geometry(), Err(RingError::Geometry));
    assert_eq!(ring.entry(0), Err(RingError::Geometry));
    Ok(())
}

#[test]
fn default_algorithms_use_current_registers_without_publishing_pointers() -> Result<(), RingError> {
    // Given: a downstream ring owns registers, not a contract ring object.
    let mut ring = RegisterRing {
        registers: CommandRegistersValues::default(),
    };
    ring.registers.base = pdma_model_contract::register::AddressValues::new(0x1000);
    ring.registers.capacity = pdma_model_contract::register::WordValues::new(2);
    let pointers = RingPointers {
        producer: 1,
        consumer: 0,
    };
    // When: the same ring changes its own configuration between calculations.
    let first = ring.entry(3)?;
    let next = ring.advance_producer(pointers, 1)?;
    ring.registers.base = pdma_model_contract::register::AddressValues::new(0x2000);
    let second = ring.entry(3)?;
    // Then: addresses follow the owner; calculating a cursor does not publish it.
    assert_eq!((first, second), (0x1070, 0x2070));
    assert_eq!(
        next,
        RingPointers {
            producer: 2,
            consumer: 0
        }
    );
    assert_eq!(ring.registers.producer.raw(), 0);
    Ok(())
}

#[test]
fn snapshot_validation_checks_wrapping_occupancy_without_publication() {
    // Given: two slots backed by a downstream register owner.
    let mut ring = RegisterRing {
        registers: CommandRegistersValues::default(),
    };
    ring.registers.capacity = pdma_model_contract::register::WordValues::new(2);
    for (producer, consumer, valid) in [
        (0, 0, true),
        (2, 0, true),
        (u32::MAX, u32::MAX, true),
        (0, u32::MAX, true),
        (1, u32::MAX, true),
        (2, u32::MAX, false),
        (0, 1, false),
    ] {
        // When: validating a snapshot without requesting an advance.
        let result = ring.validate_pointers(RingPointers { producer, consumer });
        // Then: occupancy fits capacity, including across wrap, and owner state is untouched.
        assert_eq!(
            result,
            if valid {
                Ok(())
            } else {
                Err(RingError::Pointers)
            }
        );
        assert_eq!(ring.registers.producer.raw(), 0);
        assert_eq!(ring.registers.consumer.raw(), 0);
    }
}
