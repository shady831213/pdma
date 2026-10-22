//! Explicit endpoint ranges do not require equal backing extents.

/// Shared hardware fixtures.
pub mod support;

use hardware_interface_runtime::{MemoryAccess, MemoryAlignment};
use pdma_model_contract::dma::{Endpoint, Event, EventCode, Position, Segment};
use pdma_model_contract::dma::{ErrorInfo, EventInfoValues};
use support::{TestResult, command, run, setup, submit};

#[test]
fn selected_ranges_skip_unused_sg_prefix_and_clip_last_segment() -> TestResult {
    // Given: the table's first entry is outside mapped memory; only entries 1-2 matter.
    let (mut dma, memory) = setup(0x1100, 2)?;
    memory.write(0x1200.into(), MemoryAlignment::ONE, b"abcdefgh")?;
    memory.write(0x1300.into(), MemoryAlignment::ONE, b"--------")?;
    let mut request = command();
    crate::support::copy_mut(&mut request)?.destination = Endpoint::linear(0x1300, 2);
    crate::support::copy_mut(&mut request)?.copy_length = 4;
    memory.add_regions(&[hardware_interface_runtime::MemoryRegion::try_new(
        0x3000.into(),
        32,
        MemoryAlignment::ONE,
    )?])?;
    memory.write(
        0x3000.into(),
        MemoryAlignment::ONE,
        &Segment {
            address: 0x1200,
            length: 3,
        }
        .to_bytes()?,
    )?;
    memory.write(
        0x3010.into(),
        MemoryAlignment::ONE,
        &Segment {
            address: 0x1203,
            length: 5,
        }
        .to_bytes()?,
    )?;
    crate::support::copy_mut(&mut request)?.source = Endpoint::scatter_gather(0x2ff0, 3);
    crate::support::copy_mut(&mut request)?.source = crate::support::copy(&request)?
        .source
        .with_start(Position::scatter_gather(1, 1))
        .ok_or("position mode mismatch")?;
    submit(&mut dma, &memory, request)?;
    // When: the selected four-byte range is copied.
    run(&mut dma)?;
    // Then: neither unused prefix metadata nor bytes beyond the selected range are accessed.
    assert_eq!(
        memory.read_typed::<[u8; 8]>(0x1300.into(), MemoryAlignment::ONE)?,
        *b"--bcde--"
    );
    support::assert_no_event(&mut dma)?;
    Ok(())
}

#[test]
fn invalid_selected_positions_and_ranges_fail_before_payload_writes() -> TestResult {
    for (source, expected) in [
        (Endpoint::linear(u64::MAX, 1), EventCode::InvalidLength),
        (Endpoint::linear(u64::MAX - 2, 0), EventCode::InvalidLength),
        (
            Endpoint::scatter_gather(0x1400, 1)
                .with_start(Position::scatter_gather(0, 5))
                .ok_or("SG position")?,
            EventCode::InvalidLength,
        ),
        (
            Endpoint::scatter_gather(0x1400, 1)
                .with_start(Position::scatter_gather(2, 0))
                .ok_or("SG position")?,
            EventCode::LengthMismatch,
        ),
        (
            Endpoint::scatter_gather(0x1400, 1)
                .with_start(Position::scatter_gather(0, 3))
                .ok_or("SG position")?,
            EventCode::LengthMismatch,
        ),
    ] {
        // Given: an invalid start or a selected range shorter than the requested copy.
        let (mut dma, memory) = setup(0x1100, 2)?;
        memory.write(
            0x1400.into(),
            MemoryAlignment::ONE,
            &Segment {
                address: 0x1200,
                length: 4,
            }
            .to_bytes()?,
        )?;
        memory.write(0x1300.into(), MemoryAlignment::ONE, b"keep")?;
        let mut request = command();
        crate::support::copy_mut(&mut request)?.source = source;
        // When: range preflight runs through the real pipeline.
        submit(&mut dma, &memory, request)?;
        run(&mut dma)?;
        // Then: a typed stopping result reports no committed bytes and the destination is intact.
        let raw = memory.read_typed::<pdma_model_contract::dma::EventRaw>(
            0x1100.into(),
            MemoryAlignment::ONE,
        )?;
        assert_eq!(
            Event::from_bytes(raw.as_ref())?,
            Event {
                id: 7,
                code: expected,
                info: match expected {
                    EventCode::InvalidLength => EventInfoValues::InvalidLength(ErrorInfo::ZERO),
                    EventCode::LengthMismatch => EventInfoValues::LengthMismatch(ErrorInfo::ZERO),
                    _ => return Err("unexpected range-test status".into()),
                },
            }
        );
        assert_eq!(
            memory.read_typed::<[u8; 4]>(0x1300.into(), MemoryAlignment::ONE)?,
            *b"keep"
        );
    }
    Ok(())
}

#[test]
fn segment_end_start_skips_empty_entries_without_reading_unused_suffix() -> TestResult {
    // Given: a start at the end of a segment, an empty entry, and a huge unused suffix.
    let (mut dma, memory) = setup(0x1100, 2)?;
    for (index, segment) in [
        Segment {
            address: u64::MAX,
            length: 3,
        },
        Segment {
            address: u64::MAX,
            length: 0,
        },
        Segment {
            address: 0x1200,
            length: 5,
        },
    ]
    .iter()
    .enumerate()
    {
        memory.write(
            (0x1400 + u64::try_from(index * Segment::RECORD_SIZE)?).into(),
            MemoryAlignment::ONE,
            &segment.to_bytes()?,
        )?;
    }
    memory.write(0x1200.into(), MemoryAlignment::ONE, b"abcde")?;
    let mut request = command();
    crate::support::copy_mut(&mut request)?.copy_length = 3;
    crate::support::copy_mut(&mut request)?.source = Endpoint::scatter_gather(0x1400, u64::MAX)
        .with_start(Position::scatter_gather(0, 3))
        .ok_or("SG position")?;
    // When: only the requested three bytes are copied.
    submit(&mut dma, &memory, request)?;
    run(&mut dma)?;
    // Then: empty and exhausted entries need no payload address; unused entries are not fetched.
    assert_eq!(
        memory.read_typed::<[u8; 4]>(0x1300.into(), MemoryAlignment::ONE)?,
        [b'a', b'b', b'c', 0]
    );
    support::assert_no_event(&mut dma)?;
    Ok(())
}
