use hw_model_io::memory::{MemoryAccess, MemoryAddress, MemoryAlignment};

use crate::{Scenario, TestResult, harness};

fn bytes(report: &harness::Report, address: MemoryAddress, length: usize) -> TestResult<Vec<u8>> {
    let mut data = vec![0; length];
    report
        .storage
        .read(address, MemoryAlignment::ONE, &mut data)?;
    Ok(data)
}

fn assert_completions(report: &harness::Report, expected: &[(usize, u64, u64)]) {
    let mut completions = Vec::new();
    for (channel, response) in &report.outcome.completions {
        completions.push((channel.index(), *response));
    }
    completions.sort_unstable();
    assert_eq!(
        completions,
        expected
            .iter()
            .map(|(channel, value, _)| (*channel, *value))
            .collect::<Vec<_>>()
    );
    assert_eq!(report.outcome.channel_counts, [3, 3]);
    assert!(report.outcome.discovery_write_rejected);
    assert!(report.summary.deliveries() > 0);
    assert!(!report.pending);
}

fn assert_selected_prefix(report: &harness::Report) -> TestResult {
    let first = report
        .outcome
        .original
        .first()
        .ok_or("missing channel A result")?;
    let destination = first
        .before_reset
        .get(..first.length)
        .ok_or("missing A destination")?;
    let committed = destination
        .iter()
        .take_while(|byte| **byte == first.value)
        .count();
    assert_eq!(first.command_consumer, 1);
    assert!(
        committed > 0 && committed < first.length,
        "expected an in-flight prefix, observed {committed}/{} bytes before reset",
        first.length
    );
    assert!(
        destination
            .get(committed..)
            .ok_or("missing A suffix")?
            .iter()
            .all(|byte| *byte == 0)
    );
    assert!(
        first
            .before_reset
            .get(first.length..)
            .ok_or("missing second A destination")?
            .iter()
            .all(|byte| *byte == 0)
    );
    for queued in report.outcome.original.iter().skip(1) {
        assert_eq!(queued.command_consumer, 0);
        assert_eq!(queued.before_reset, vec![0; queued.length * 2]);
    }
    Ok(())
}

fn assert_preserved(report: &harness::Report, index: usize) -> TestResult {
    let copy = report
        .outcome
        .original
        .get(index)
        .ok_or("missing original destination")?;
    assert_eq!(
        bytes(report, copy.address, copy.length * 2)?,
        copy.before_reset
    );
    Ok(())
}

fn assert_completed(report: &harness::Report, index: usize) -> TestResult {
    let copy = report
        .outcome
        .original
        .get(index)
        .ok_or("missing original destination")?;
    let mut expected = vec![copy.value; copy.length];
    expected.resize(copy.length * 2, 0);
    assert_eq!(bytes(report, copy.address, copy.length * 2)?, expected);
    Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn resetting_inflight_channel_preserves_queued_work_and_accepts_fresh_work() -> TestResult {
    // Given: A owns the shared execution unit while B/C have queued commands.
    // When: software resets A after its first payload write and submits fresh A work.
    let report = harness::run(Scenario::CopyInFlight).await?;
    // Then: old A stops at its committed prefix; queued B/C and fresh A complete exactly once.
    assert_selected_prefix(&report)?;
    assert_preserved(&report, 0)?;
    assert_completed(&report, 1)?;
    assert_completed(&report, 2)?;
    assert_completions(&report, &[(0, 20, 1024), (1, 11, 16_384), (2, 12, 16_384)]);
    assert_eq!(
        bytes(&report, report.outcome.fresh, 1024)?,
        vec![0x31; 1024]
    );
    Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn resetting_queued_channel_preserves_selected_copy_and_other_queued_work() -> TestResult {
    // Given: A has committed a payload prefix and B/C remain queued.
    // When: software resets queued B and submits a fresh B command.
    let report = harness::run(Scenario::QueuedChannel).await?;
    // Then: A finishes exactly, old B stays untouched, C and fresh B complete exactly once.
    assert_selected_prefix(&report)?;
    assert_completed(&report, 0)?;
    assert_preserved(&report, 1)?;
    assert_completed(&report, 2)?;
    assert_completions(&report, &[(0, 10, 1024), (1, 20, 16_384), (2, 12, 16_384)]);
    assert_eq!(
        bytes(&report, report.outcome.fresh, 16_384)?,
        vec![0x32; 16_384]
    );
    Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn successful_commands_do_not_consume_event_capacity() -> TestResult {
    // Given: A has one event slot and two copies; B/C have one copy each.
    // When: software waits for their Fence values, then resets A.
    let report = harness::run(Scenario::SuccessWithoutEvents).await?;
    // Then: both A copies finish without events, and peers and fresh A complete.
    let (events, consumed) = report
        .outcome
        .blocked_cursors
        .ok_or("missing cursor snapshot")?;
    assert_eq!(events, 0);
    assert!((3..=4).contains(&consumed));
    let first = report
        .outcome
        .original
        .first()
        .ok_or("missing channel A result")?;
    let mut expected = vec![first.value; first.length];
    expected.resize(first.length * 2, first.value);
    assert_eq!(first.before_reset, expected);
    assert_preserved(&report, 0)?;
    for index in 1..3 {
        let peer = report
            .outcome
            .original
            .get(index)
            .ok_or("missing peer result")?;
        let mut completed = vec![peer.value; peer.length];
        completed.resize(peer.length * 2, 0);
        assert_eq!(peer.before_reset, completed);
        assert_completed(&report, index)?;
    }
    assert_completions(&report, &[(0, 20, 1024), (1, 11, 16_384), (2, 12, 16_384)]);
    assert_eq!(
        bytes(&report, report.outcome.fresh, 1024)?,
        vec![0x31; 1024]
    );
    Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn device_reset_cancels_selected_and_queued_work_and_preserves_discovery() -> TestResult {
    // Given: A owns the execution unit and B/C have unconsumed commands.
    // When: software resets the whole device during A's copy and configures fresh A work.
    let report = harness::run(Scenario::WholeDevice).await?;
    // Then: A's prefix is preserved, B/C remain untouched, and only fresh A completes.
    assert_selected_prefix(&report)?;
    for index in 0..3 {
        assert_preserved(&report, index)?;
    }
    assert_completions(&report, &[(0, 20, 1024)]);
    assert_eq!(
        bytes(&report, report.outcome.fresh, 1024)?,
        vec![0x31; 1024]
    );
    Ok(())
}
