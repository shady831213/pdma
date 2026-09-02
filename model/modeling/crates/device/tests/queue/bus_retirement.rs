#![allow(missing_docs, clippy::redundant_pub_crate)]

/// Exact retirement behavior cases.
pub(super) mod cases;
pub(super) mod support;

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error + Send + Sync + 'static>>;

#[tokio::test(flavor = "current_thread")]
async fn dma_result_is_visible_before_irq_and_retries_without_reexecution() -> TestResult {
    cases::visible_before_irq_and_retry().await
}

#[tokio::test(flavor = "current_thread")]
async fn full_or_failed_result_publication_keeps_one_pending_record_without_irq() -> TestResult {
    cases::publication_pressure_keeps_one_pending().await
}

#[tokio::test(flavor = "current_thread")]
async fn corrupt_result_masked_line_and_stale_message_preserve_consumers() -> TestResult {
    cases::corrupt_masked_and_stale().await
}
