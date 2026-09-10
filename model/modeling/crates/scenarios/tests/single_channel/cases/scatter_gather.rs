//! Independent endpoint layouts exercised through the complete software program.

use super::{TestResult, copy};
use crate::software;

#[tokio::test(flavor = "current_thread")]
async fn all_endpoint_combinations_copy_across_independent_segment_boundaries() -> TestResult {
    // Given: the same byte stream with independent source and destination boundaries.
    let sources = [
        software::BufferLayout::Linear,
        software::BufferLayout::ScatterGather(&[3, 0, 5]),
    ];
    let destinations = [
        software::BufferLayout::Linear,
        software::BufferLayout::ScatterGather(&[2, 6]),
    ];
    for source in sources {
        for destination in destinations {
            // When: software builds each endpoint and waits for its completion ISR.
            copy(
                false,
                software::Input {
                    data: b"dma-copy",
                    source,
                    destination,
                },
            )
            .await?;
            // Then: shared case assertions verify backing bytes, completed count and IRQ delivery.
        }
    }
    Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn scatter_gather_copy_exceeds_the_internal_chunk_size() -> TestResult {
    // Given: 769 bytes and segment boundaries crossing multiple internal chunks.
    let data: Vec<u8> = (0..=255)
        .chain((0..=255).rev())
        .chain(0..=255)
        .chain([37])
        .collect();
    let input = software::Input {
        data: &data,
        source: software::BufferLayout::ScatterGather(&[0, 257, 0, 512, 0]),
        destination: software::BufferLayout::ScatterGather(&[128, 0, 385, 256]),
    };
    // When: one command transfers the entire logical stream.
    copy(false, input).await?;
    // Then: backing bytes and bytes_completed cover all 769 bytes.
    Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn zero_total_endpoints_complete_without_payload_mappings() -> TestResult {
    // Given: zero-byte allocations are unmapped, with empty or all-zero SG lists.
    let layouts = [
        software::BufferLayout::Linear,
        software::BufferLayout::ScatterGather(&[]),
        software::BufferLayout::ScatterGather(&[0, 0, 0]),
    ];
    for source in layouts {
        for destination in layouts {
            // When: software submits each independent pair and awaits its ISR.
            copy(
                false,
                software::Input {
                    data: &[],
                    source,
                    destination,
                },
            )
            .await?;
            // Then: success reports zero completed bytes and the IRQ is drained.
        }
    }
    Ok(())
}
