//! Candidate selection is a value policy, independent of channel hardware.

use hardware_interface_pipeline::Arbitration;
use pdma_model_contract::dma::ChannelConfiguration;
use pdma_model_contract::dma::{
    ChannelBlock, ChannelId, ChannelRegisters, CommandRegisters, EventRegisters,
};
use pdma_model_dma::{Dma, RoundRobin};

#[test]
fn round_robin_rotates_completed_eligible_values_and_skips_idle_channels()
-> Result<(), Box<dyn std::error::Error>> {
    // Given: completed channel snapshots with one idle channel.
    let mut policy = RoundRobin::default();
    let mut device = Dma::with_sideband((), 3, std::convert::identity)?;
    for index in [0, 2] {
        let base = ChannelId::new(index).register_offset(3).ok_or("channel")?;
        for (offset, value) in [
            (
                ChannelBlock::COMMAND.offset() + CommandRegisters::CAPACITY.offset(),
                1_u32,
            ),
            (
                ChannelBlock::EVENT.offset() + EventRegisters::CAPACITY.offset(),
                1,
            ),
            (
                ChannelBlock::CONTROL.offset()
                    + ChannelRegisters::CONFIG.offset()
                    + ChannelConfiguration::ENABLE.offset(),
                1,
            ),
            (
                ChannelBlock::COMMAND.offset() + CommandRegisters::PRODUCER.offset(),
                1,
            ),
        ] {
            device.write(base + offset, &value.to_le_bytes())?;
        }
    }
    let values = [
        Some(device.command_request(ChannelId::new(0))?),
        Some(None),
        Some(device.command_request(ChannelId::new(2))?),
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
    Ok(())
}
