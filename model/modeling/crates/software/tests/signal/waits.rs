use super::memory;
use crate::TestResult;
use hardware_interface_runtime::{AtomicWriteOrder, MemoryAccess, MemoryAlignment};
use pdma_model_contract::dma::{WaitCondition, WaitOrder};
use pdma_model_software::signal::Signal;
use std::{
    future::Future,
    task::{Context, Poll, Waker},
};

#[test]
fn try_wait_uses_signed_comparisons_at_both_extremes() -> TestResult {
    for observed in [i64::MIN, -1, 0, 1, i64::MAX] {
        for compare in [i64::MIN, -1, 0, 1, i64::MAX] {
            for condition in [
                WaitCondition::Equal,
                WaitCondition::NotEqual,
                WaitCondition::LessThan,
                WaitCondition::GreaterOrEqual,
            ] {
                // Given: a full-width signed value written through ordinary memory.
                let memory = memory()?;
                memory.write(0x1000.into(), MemoryAlignment::ONE, &observed.to_le_bytes())?;
                let signal = Signal::new(&memory, 0x1000.into());
                let matches = match condition {
                    WaitCondition::Equal => observed == compare,
                    WaitCondition::NotEqual => observed != compare,
                    WaitCondition::LessThan => observed < compare,
                    WaitCondition::GreaterOrEqual => observed >= compare,
                };
                // When: both permitted wait orders inspect the condition.
                for order in [WaitOrder::Relaxed, WaitOrder::Acquire] {
                    let result = signal.try_wait(condition, compare, order)?;
                    // Then: only satisfying observations are returned.
                    assert_eq!(result, matches.then_some(observed));
                }
            }
        }
    }
    Ok(())
}

#[tokio::test]
async fn cooperative_wait_observes_payload_after_release_publication() -> TestResult {
    // Given: a signal initially unequal to the expected publication value.
    let memory = memory()?;
    let signal = Signal::new(&memory, 0x1000.into());
    let payload = [3, 17, 29, 41, 53, 67, 79, 97];
    let expected_value = -37;
    let waiter = async {
        let observed = signal
            .wait(WaitCondition::Equal, expected_value, WaitOrder::Acquire)
            .await?;
        let mut bytes = [0; 8];
        memory.read(0x1008.into(), MemoryAlignment::ONE, &mut bytes)?;
        Ok::<_, Box<dyn std::error::Error>>((observed, bytes))
    };
    let publisher = async {
        memory.write(0x1008.into(), MemoryAlignment::ONE, &payload)?;
        signal.store(expected_value, AtomicWriteOrder::Release)?;
        Ok::<_, Box<dyn std::error::Error>>(())
    };
    // When: the unmet waiter runs first and must yield to the publisher.
    let (result, ()) = tokio::try_join!(biased; waiter, publisher)?;
    // Then: the satisfying acquire observation exposes preceding ordinary writes.
    assert_eq!(result, (expected_value, payload));
    Ok(())
}

#[test]
fn cancelling_a_pending_wait_leaves_storage_unchanged() -> TestResult {
    // Given: an unsatisfied signal and a borrowed wait future.
    let memory = memory()?;
    let signal = Signal::new(&memory, 0x1000.into());
    let mut context = Context::from_waker(Waker::noop());
    // When: one poll yields, then the wait is dropped.
    {
        let mut wait = std::pin::pin!(signal.wait(WaitCondition::Equal, -1, WaitOrder::Relaxed));
        assert!(matches!(wait.as_mut().poll(&mut context), Poll::Pending));
    }
    // Then: cancellation had no writes or retained ownership blocking a later waiter.
    assert_eq!(
        signal.try_wait(WaitCondition::Equal, 0, WaitOrder::Relaxed)?,
        Some(0)
    );
    signal.store(-1, AtomicWriteOrder::Relaxed)?;
    let mut wait = std::pin::pin!(signal.wait(WaitCondition::Equal, -1, WaitOrder::Relaxed));
    assert!(matches!(
        wait.as_mut().poll(&mut context),
        Poll::Ready(Ok(-1))
    ));
    Ok(())
}
