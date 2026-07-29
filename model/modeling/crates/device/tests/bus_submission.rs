#![allow(missing_docs, unreachable_pub, clippy::redundant_pub_crate)]

#[path = "bus_submission/boundaries.rs"]
mod boundaries;
#[path = "bus_submission/common.rs"]
mod common;
#[path = "bus_submission/faults.rs"]
mod faults;
#[path = "bus_submission/operations.rs"]
mod operations;
#[path = "bus_submission/precedence.rs"]
mod precedence;
#[path = "bus_submission/recovery.rs"]
mod recovery;

use boundaries::capture_and_nonzero_slot;
use faults::{BroadcastFaultCase, assert_broadcast_fault, malformed_and_resource_stall};
use operations::{OperationCase, execute_operation, multi_queue_arbitration};
use precedence::{DirectFaultCase, HeaderFaultCase, assert_direct_fault, assert_header_fault};
use recovery::{dma_and_semantic_queues_coexist, reset_discards_pending_dma_retirement};

type TestResult = Result<(), Box<dyn std::error::Error + Send + Sync + 'static>>;

#[tokio::test(flavor = "current_thread")]
async fn all_operations_fetch_from_dma_and_match_semantic_core() -> TestResult {
    // Given: one accepted DMA descriptor for each existing operation.
    // When: each captured producer is fetched through the DMA queue frontend.
    // Then: the shared semantic engine produces the exact destination bytes and retirement.
    for operation in OperationCase::ALL {
        execute_operation(operation).await?;
    }
    capture_and_nonzero_slot().await?;
    multi_queue_arbitration().await?;
    dma_and_semantic_queues_coexist().await?;
    Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn malformed_descriptor_list_and_resource_pressure_preserve_commit_points() -> TestResult {
    // Given: published malformed descriptor/list bytes and a valid resource-starved command.
    // When: the worker fetches each captured producer.
    // Then: malformed slots consume once while resource pressure consumes and writes nothing.
    malformed_and_resource_stall().await?;
    reset_discards_pending_dma_retirement().await
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
