//! Channel notification identity at the hardware/platform boundary.

use pdma_model_contract::dma::{ChannelId, CommandHeader, Opcode, TrapCommand};
/// Shared DMA fixtures.
pub mod support;
use support::{TestResult, run, setup, submit};

#[test]
fn channel_notification_levels_are_independent() -> TestResult {
    // Given: a completed command with an unclaimed channel-zero notification.
    let (mut dma, memory) = setup(0x1100, 2)?;
    submit(
        &mut dma,
        &memory,
        pdma_model_contract::dma::Command::Trap(TrapCommand {
            header: CommandHeader {
                opcode: Opcode::Trap,
            },
            id: 7,
        }),
    )?;
    run(&mut dma)?;
    // When: the platform queries a different channel and the device-wide source.
    assert!(!dma.channel_irq(ChannelId::new(1)));
    assert!(!dma.device_irq());
    // Then: the original source remains pending and remains asserted after observation.
    assert!(dma.channel_irq(ChannelId::new(0)));
    assert!(dma.irq());
    Ok(())
}
