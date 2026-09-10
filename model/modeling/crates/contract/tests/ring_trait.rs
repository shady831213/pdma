//! Downstream rings supply their own state and reuse the contract algorithms.

use pdma_model_contract::{Command, CommandRegistersValues, Ring, RingError, RingPointers};

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
    assert_eq!(stride, 56);
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
    ring.registers.base = pdma_model_contract::AddressValues::new(0x1000);
    ring.registers.capacity = pdma_model_contract::WordValues::new(2);
    let pointers = RingPointers {
        producer: 1,
        consumer: 0,
    };
    // When: the same ring changes its own configuration between calculations.
    let first = ring.entry(3)?;
    let next = ring.advance_producer(pointers, 1)?;
    ring.registers.base = pdma_model_contract::AddressValues::new(0x2000);
    let second = ring.entry(3)?;
    // Then: addresses follow the owner; calculating a cursor does not publish it.
    assert_eq!((first, second), (0x1038, 0x2038));
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
