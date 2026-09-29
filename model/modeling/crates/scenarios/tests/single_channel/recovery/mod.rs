//! System acceptance for batched notifications and software channel recovery.

mod hardware;
mod software;

use crate::harness::TestResult;
use hardware_interface_runtime::{MemoryAccess, MemoryAlignment};
use pdma_model_contract::dma::{ChannelFatalCode, CompletionFlagsValues, Response, ResponseStatus};
use pdma_model_contract::dma::{CompletionInfo, ResponseInfoValues};

#[derive(Clone, Copy, Debug)]
enum Scenario {
    Batch,
    ChannelReset,
}

#[tokio::test(flavor = "current_thread")]
async fn recovery_batch_last_ioc_notifies_once_and_drains_all_responses() -> TestResult {
    // Given: three copies with nonempty notifications masked and IOC on the last only.
    let payload = b"batch-copy";
    // When: the complete software program submits and drains through its registered ISR.
    let report = hardware::run(Scenario::Batch, payload).await?;
    // Then: one notification exposes every response and every destination's copied bytes.
    assert_eq!(report.summary.deliveries(), 1);
    assert!(report.worker.is_cancelled());
    assert_eq!(report.outcome.responses.len(), 3);
    for (index, response) in report.outcome.responses.iter().enumerate() {
        assert_eq!(
            *response,
            Response {
                id: 40 + u64::try_from(index)?,
                status: ResponseStatus::Success,
                flags: CompletionFlagsValues { ioc: index == 2 },
                info: ResponseInfoValues::Success(CompletionInfo {
                    bytes_completed: u64::try_from(payload.len())?
                }),
            }
        );
    }
    assert_eq!(report.outcome.causes.len(), 1);
    let cause = report.outcome.causes.first().ok_or("missing batch cause")?;
    assert!(cause.response_non_empty());
    assert!(cause.ioc());
    assert!(!cause.response_error());
    let mut copied = vec![0; payload.len() * 3];
    report.storage.read(
        report.outcome.destination,
        MemoryAlignment::ONE,
        &mut copied,
    )?;
    assert_eq!(copied, payload.repeat(3));
    assert!(!report.irq);
    Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn recovery_error_irq_allows_channel_reset_and_next_copy_in_same_runtime() -> TestResult {
    // Given: a length-mismatched command with both routine notification kinds masked.
    let payload = b"after-reset";
    // When: software handles the error, resets, rebuilds its driver and submits a valid copy.
    let report = hardware::run(Scenario::ChannelReset, payload).await?;
    // Then: the error bypassed masks and a fresh nonempty event completed the next copy.
    assert_eq!(report.summary.deliveries(), 2);
    assert!(report.worker.is_cancelled());
    assert_eq!(
        report.outcome.responses,
        vec![
            Response {
                id: 41,
                status: ResponseStatus::LengthMismatch,
                flags: CompletionFlagsValues::default(),
                info: ResponseInfoValues::LengthMismatch(CompletionInfo { bytes_completed: 0 }),
            },
            Response {
                id: 42,
                status: ResponseStatus::Success,
                flags: CompletionFlagsValues::default(),
                info: ResponseInfoValues::Success(CompletionInfo {
                    bytes_completed: u64::try_from(payload.len())?
                }),
            },
        ]
    );
    assert_eq!(report.outcome.fatal, ChannelFatalCode::None);
    assert_eq!(report.outcome.causes.len(), 2);
    let first = report.outcome.causes.first().ok_or("missing error cause")?;
    assert!(first.response_error());
    let second = report
        .outcome
        .causes
        .get(1)
        .ok_or("missing recovered cause")?;
    assert!(second.response_non_empty());
    assert!(!second.response_error());
    assert!(!second.ioc());
    let mut copied = vec![0; payload.len()];
    report.storage.read(
        report.outcome.destination,
        MemoryAlignment::ONE,
        &mut copied,
    )?;
    assert_eq!(copied, payload);
    assert!(!report.irq);
    Ok(())
}
