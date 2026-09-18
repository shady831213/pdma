//! Channel notification identity at the hardware/platform boundary.
/// Shared DMA fixtures.
pub mod support;
use pdma_model_contract::ChannelId;
use support::{TestResult, command, run, setup, submit};

#[test]
fn channel_notification_is_not_consumed_by_another_destination() -> TestResult {
    // Given: a completed command with an unclaimed channel-zero notification.
    let (mut dma, memory) = setup(0x1100, 2)?;
    submit(&mut dma, &memory, command())?;
    run(&mut dma)?;
    // When: the platform queries a different channel and the device-wide source.
    assert!(!dma.take_channel_interrupt(ChannelId::new(1)));
    assert!(!dma.take_device_interrupt());
    // Then: the original source remains pending and is consumed exactly once.
    assert!(dma.take_channel_interrupt(ChannelId::new(0)));
    assert!(!dma.take_channel_interrupt(ChannelId::new(0)));
    assert!(!dma.take_interrupt());
    Ok(())
}
