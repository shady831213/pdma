#![allow(missing_docs, unreachable_pub, clippy::redundant_pub_crate)]

mod boundaries;
pub(super) mod common;
mod faults;
pub(super) mod operations;
mod precedence;

use boundaries::capture_and_nonzero_slot;
use faults::{BroadcastFaultCase, assert_broadcast_fault, malformed_and_resource_stall};
use operations::{OperationCase, execute_operation, zero_row_strided};
use precedence::{DirectFaultCase, HeaderFaultCase, assert_direct_fault, assert_header_fault};

type TestResult = Result<(), Box<dyn std::error::Error + Send + Sync + 'static>>;

#[tokio::test(flavor = "current_thread")]
async fn all_operations_fetch_from_dma_and_publish_exact_records() -> TestResult {
    // Given: one accepted DMA descriptor for each existing operation.
    // When: each captured producer is fetched through the DMA queue frontend.
    // Then: the execution core produces exact destination bytes and DMA retirement records.
    for operation in OperationCase::ALL {
        execute_operation(operation).await?;
    }
    capture_and_nonzero_slot().await?;
    zero_row_strided().await?;
    Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn malformed_descriptor_list_and_resource_pressure_preserve_commit_points() -> TestResult {
    // Given: published malformed descriptor/list bytes and a valid resource-starved command.
    // When: the worker fetches each captured producer.
    // Then: malformed slots consume once while resource pressure leaves cursors and bytes intact.
    malformed_and_resource_stall().await
}

#[tokio::test(flavor = "current_thread")]
async fn indirect_fault_precedence_distinguishes_count_alignment_overflow_unmapped_and_bounds()
-> TestResult {
    // Given: broadcast descriptors with conflicting indirect-list failure classes.
    // When: each descriptor is classified at the untrusted DMA boundary.
    // Then: the canonical selector returns the exact first count/alignment/range/mapping fault.
    for case in BroadcastFaultCase::ALL {
        assert_broadcast_fault(case).await?;
    }
    for case in HeaderFaultCase::ALL {
        assert_header_fault(case).await?;
    }
    for case in DirectFaultCase::ALL {
        assert_direct_fault(case).await?;
    }
    Ok(())
}
