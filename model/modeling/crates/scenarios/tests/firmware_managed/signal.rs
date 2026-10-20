use crate::harness::{self, TestResult, program::signal::Input};
use pdma_model_contract::dma::ResponseStatus;

#[tokio::test]
async fn managed_signal_wait_drain_is_pending_until_channel_reset_and_control_recovers()
-> TestResult {
    use pdma_model_contract::control::ControlResponseStatus;
    // Given: an unmet Wait followed by an update in a firmware-managed PASID context.
    let input = Input {
        initial: -5,
        release: 7,
        increment: 1,
        decoys: [101, 202, 303],
        ids: [30, 31, 32, 33],
    };
    // When: software pipelines Drain, a busy probe and Reset, then submits fresh work.
    let outcome = harness::signal::run_recovery(input).await?;
    // Then: reset stops the Drain, discards old work, and retains a usable grant/context.
    assert!(outcome.wait_pending);
    assert_eq!(outcome.probe.id, outcome.requests[1]);
    assert_eq!(outcome.probe.status, ControlResponseStatus::ContextBusy);
    assert_eq!(outcome.drain.id, outcome.requests[0]);
    assert_eq!(
        outcome.drain.status,
        ControlResponseStatus::MaintenanceFailed
    );
    assert_eq!(
        outcome.drain.channel,
        u32::try_from(outcome.channel.index())?
    );
    assert_eq!(outcome.reset.id, outcome.requests[2]);
    assert_eq!(outcome.reset.status, ControlResponseStatus::Success);
    assert_eq!(outcome.before_fresh, input.initial);
    assert_eq!(outcome.fresh.id, input.ids[2]);
    assert_eq!(outcome.fresh.status, ResponseStatus::SignalSuccess);
    assert_eq!(outcome.fresh.signal_value(), Some(input.initial));
    assert_eq!(
        outcome.final_value,
        input.initial.wrapping_add(input.increment)
    );
    Ok(())
}

#[tokio::test]
async fn managed_signal_releases_peer_wait_in_vf_pasid_without_successor_overtaking() -> TestResult
{
    // Given: identical addresses in PF/VF Default/PASID-zero spaces and two VF channels.
    let input = Input {
        initial: -5,
        release: 7,
        increment: 1,
        decoys: [101, 202, 303],
        ids: [10, 11, 20, 21],
    };
    // When: firmware grants contexts/channels and VF software queues Wait, successor and peer Signal.
    let (pf_decoys, outcome) = harness::signal::run(input).await?;
    // Then: peer completion precedes release; Wait orders its successor and only VF PASID-zero changes.
    assert!(outcome.same_address);
    assert!(outcome.successor_blocked);
    assert_eq!(outcome.before_release, input.initial);
    assert_eq!(pf_decoys, [input.decoys[0], input.decoys[1]]);
    assert_eq!(outcome.default_decoy, input.decoys[2]);
    assert_eq!(outcome.final_value, input.release + input.increment);
    let expected = [
        (input.ids[2], input.initial),
        (input.ids[3], input.initial),
        (input.ids[0], input.release),
        (input.ids[1], input.release),
    ];
    for (response, (id, value)) in outcome.responses.into_iter().zip(expected) {
        assert_eq!(response.id, id);
        assert_eq!(response.status, ResponseStatus::SignalSuccess);
        assert_eq!(response.signal_value(), Some(value));
    }
    Ok(())
}
