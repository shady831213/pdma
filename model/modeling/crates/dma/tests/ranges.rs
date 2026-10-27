//! Explicit endpoint ranges do not require equal backing extents.

use pdma_model_contract::dma::{
    CopyCommandDestinationValues, CopyCommandSourceValues, LinearEndpoint, ScatterGatherEndpoint,
};

/// Shared hardware fixtures.
pub mod support;

use hardware_interface_memory::{MemoryAccess, MemoryAlignment};
use pdma_model_contract::dma::{ErrorInfo, EventInfoValues};
use pdma_model_contract::dma::{Event, EventCode, Segment};
use support::{TestResult, command, run, setup, submit};

#[test]
fn selected_ranges_skip_unused_sg_prefix_and_clip_last_segment() -> TestResult {
    // Given: the table's first entry is outside mapped memory; only entries 1-2 matter.
    let (mut dma, memory) = setup(0x1100, 2)?;
    memory.write(0x1200.into(), MemoryAlignment::ONE, b"abcdefgh")?;
    memory.write(0x1300.into(), MemoryAlignment::ONE, b"--------")?;
    let mut request = command();
    crate::support::copy_mut(&mut request)?.destination =
        CopyCommandDestinationValues::Linear(LinearEndpoint {
            address: 0x1300,
            offset: 2,
        });
    crate::support::copy_mut(&mut request)?.copy_length = 4;
    memory.add_regions(&[hardware_interface_memory::MemoryRegion::try_new(
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
    crate::support::copy_mut(&mut request)?.source =
        CopyCommandSourceValues::ScatterGather(ScatterGatherEndpoint {
            list_address: 0x2ff0,
            count: 3,
            index: 1,
            offset: 1,
        });
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
        (
            CopyCommandSourceValues::Linear(LinearEndpoint {
                address: u64::MAX,
                offset: 1,
            }),
            EventCode::InvalidLength,
        ),
        (
            CopyCommandSourceValues::Linear(LinearEndpoint {
                address: u64::MAX - 2,
                offset: 0,
            }),
            EventCode::InvalidLength,
        ),
        (
            CopyCommandSourceValues::ScatterGather(ScatterGatherEndpoint {
                list_address: 0x1400,
                count: 1,
                index: 0,
                offset: 5,
            }),
            EventCode::InvalidLength,
        ),
        (
            CopyCommandSourceValues::ScatterGather(ScatterGatherEndpoint {
                list_address: 0x1400,
                count: 1,
                index: 2,
                offset: 0,
            }),
            EventCode::LengthMismatch,
        ),
        (
            CopyCommandSourceValues::ScatterGather(ScatterGatherEndpoint {
                list_address: 0x1400,
                count: 1,
                index: 0,
                offset: 3,
            }),
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
    crate::support::copy_mut(&mut request)?.source =
        CopyCommandSourceValues::ScatterGather(ScatterGatherEndpoint {
            list_address: 0x1400,
            count: u64::MAX,
            index: 0,
            offset: 3,
        });
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
