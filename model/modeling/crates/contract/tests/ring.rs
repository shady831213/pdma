//! Shared entry addressing and pointer rules independent of memory and bus backends.

use core::marker::PhantomData;
use hardware_interface_schema::{RecordLayout, record_schema};
use pdma_model_contract::{Command, Response, Ring, RingError, RingPointers};

struct LayoutRing<Entry>(u64, u32, PhantomData<Entry>);

impl<Entry> LayoutRing<Entry> {
    const fn new(base: u64, capacity: u32) -> Self {
        Self(base, capacity, PhantomData)
    }
}

impl<Entry: RecordLayout> Ring for LayoutRing<Entry> {
    type Entry = Entry;

    fn base(&self) -> u64 {
        self.0
    }
    fn capacity(&self) -> u32 {
        self.1
    }
}

struct TestRing(u32);

impl Ring for TestRing {
    type Entry = Command;

    fn base(&self) -> u64 {
        0x1000
    }
    fn capacity(&self) -> u32 {
        self.0
    }
}

record_schema! {
    record TinyEntry {
        alignment: 1,
        endian: Little,
        fields: { first: u8, second: u8, third: u8 },
    }
}

record_schema! {
    record AlignedEntry {
        alignment: 8,
        endian: Little,
        fields: { payload: u8 },
    }
}

#[test]
fn entries_follow_wire_layout_and_wrap_for_different_record_types() -> Result<(), RingError> {
    // Given
    let commands = LayoutRing::<Command>::new(0x1000, 4);
    let responses = LayoutRing::<Response>::new(0x2000, 4);
    let tiny = LayoutRing::<TinyEntry>::new(0x3000, 4);
    let aligned = LayoutRing::<AlignedEntry>::new(0x4000, 2);
    // When
    let addresses = [
        commands.entry(3)?,
        responses.entry(3)?,
        tiny.entry(2)?,
        tiny.entry(4)?,
    ];
    // Then
    assert_eq!(addresses, [0x10a8, 0x2048, 0x3006, 0x3000]);
    assert_eq!(commands.entry(4)?, 0x1000);
    assert_eq!(aligned.entry(1)?, 0x4008);
    assert_eq!(commands.base(), 0x1000);
    assert_eq!(commands.capacity(), 4);
    Ok(())
}

#[test]
fn invalid_geometry_is_rejected_before_entry_arithmetic() {
    for (base, capacity) in [
        (0x1000, 0),
        (0x1000, 3),
        (0x1001, 1),
        (0x1000, 1 << 31),
        (u64::MAX - 7, 2),
    ] {
        // Given: untrusted base and capacity.
        // When
        let result = LayoutRing::<Command>::new(base, capacity).entry(0);
        // Then
        assert!(matches!(result, Err(RingError::Geometry)));
    }
}

#[test]
fn pointer_advancement_wraps_without_confusing_full_and_empty() -> Result<(), RingError> {
    // Given
    let empty = RingPointers {
        producer: u32::MAX,
        consumer: u32::MAX,
    };
    // When
    let full = TestRing(2).advance_producer(empty, 2)?;
    let drained = TestRing(2).advance_consumer(full, 2)?;
    // Then
    assert_eq!(
        full,
        RingPointers {
            producer: 1,
            consumer: u32::MAX
        }
    );
    assert_eq!(full.pending(), 2);
    assert!(!full.is_empty());
    assert_eq!(
        drained,
        RingPointers {
            producer: 1,
            consumer: 1
        }
    );
    assert!(drained.is_empty());
    assert_eq!(
        TestRing(2).advance_producer(full, 1),
        Err(RingError::Pointers)
    );
    assert_eq!(
        TestRing(2).advance_consumer(drained, 1),
        Err(RingError::Pointers)
    );
    Ok(())
}

#[test]
fn raw_pointer_publication_rejects_retreat_and_overcommit() {
    // Given
    let pointers = RingPointers {
        producer: 11,
        consumer: 10,
    };
    // When
    let results = [
        TestRing(2).with_producer(pointers, 10),
        TestRing(2).with_producer(pointers, 13),
        TestRing(2).with_consumer(pointers, 9),
        TestRing(2).with_consumer(pointers, 12),
        TestRing(0).with_producer(pointers, 11),
    ];
    // Then
    assert_eq!(results, [Err(RingError::Pointers); 5]);
    assert_eq!(
        TestRing(2).with_producer(pointers, 12),
        Ok(RingPointers {
            producer: 12,
            consumer: 10
        })
    );
    assert_eq!(
        TestRing(2).with_consumer(pointers, 11),
        Ok(RingPointers {
            producer: 11,
            consumer: 11
        })
    );
}

#[test]
fn shrinking_capacity_cannot_hide_published_entries() {
    // Given
    let full = RingPointers {
        producer: 2,
        consumer: 0,
    };
    // When
    let result = TestRing(1).with_producer(full, 2);
    // Then
    assert_eq!(result, Err(RingError::Pointers));
}

#[test]
fn every_accepted_capacity_keeps_entries_contiguous_across_counter_overflow()
-> Result<(), RingError> {
    for capacity in 1..=8 {
        // Given
        let ring = LayoutRing::<Response>::new(0x1000, capacity);
        if ring.validate_geometry().is_ok() {
            for pointer in [u32::MAX - 1, u32::MAX] {
                let current = ring.entry(pointer)?;
                let expected = if current + 24 == 0x1000 + u64::from(capacity) * 24 {
                    0x1000
                } else {
                    current + 24
                };
                // When
                let next = ring.entry(pointer.wrapping_add(1))?;
                // Then
                assert_eq!(next, expected, "capacity {capacity}, pointer {pointer}");
            }
        }
    }
    Ok(())
}

#[test]
fn every_legal_small_ring_transition_matches_an_integer_queue() {
    for capacity in 1_u32..=8 {
        for consumer in [0, 13, u32::MAX - 3, u32::MAX] {
            for pending in 0..=capacity {
                // Given
                let pointers = RingPointers {
                    producer: consumer.wrapping_add(pending),
                    consumer,
                };
                for count in 0..=capacity + 1 {
                    // When
                    let produced = TestRing(capacity).advance_producer(pointers, count);
                    let consumed = TestRing(capacity).advance_consumer(pointers, count);
                    // Then
                    assert_eq!(produced.is_ok(), count <= capacity - pending);
                    assert_eq!(consumed.is_ok(), count <= pending);
                    if let Ok(next) = produced {
                        assert_eq!(next.pending(), pending + count);
                    }
                    if let Ok(next) = consumed {
                        assert_eq!(next.pending(), pending - count);
                    }
                }
            }
        }
    }
}
