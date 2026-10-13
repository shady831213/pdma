//! Candidate selection is a value policy, independent of channel hardware.

use hardware_interface_pipeline::Arbitration;
use pdma_model_contract::dma::ChannelId;
use pdma_model_dma::{CommandRequest, RoundRobin};

#[test]
fn round_robin_rotates_completed_eligible_values_and_skips_idle_channels() {
    // Given: completed channel snapshots with one idle channel.
    let mut policy = RoundRobin::default();
    let values = [
        Some(Some(CommandRequest::Fetch(ChannelId::new(0)))),
        Some(None),
        Some(Some(CommandRequest::Resume(ChannelId::new(2)))),
    ];
    // When: independent rounds offer the same eligible channels.
    let selected: Vec<_> = (0..4).map(|_| policy.select(&values)).collect();
    // Then: eligible channels alternate and an empty snapshot has no winner.
    assert_eq!(
        selected,
        vec![Ok(Some(0)), Ok(Some(2)), Ok(Some(0)), Ok(Some(2))]
    );
    assert_eq!(policy.select(&[Some(None), Some(None)]), Ok(None));
    assert_eq!(policy.select(&[]), Ok(None));
}
