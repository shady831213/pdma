//! Permission apertures are complete schema blocks.

use pdma_model_contract::dma::{ChannelBlock, DmaRegisters};

#[test]
fn channel_data_aperture_excludes_privileged_reset() -> Result<(), Box<dyn std::error::Error>> {
    // Given: the generated data channel and complete two-channel schema.
    let data = ChannelBlock::resolve()?;
    let dma = DmaRegisters::new(2)?.resolve()?;
    // When: enumerating the registers exposed by a complete data aperture.
    let resets = data
        .registers()
        .iter()
        .filter(|leaf| leaf.path().rsplit('.').next() == Some("reset"))
        .count();
    // Then: reset is absent from data and present in a separate administration bank.
    assert_eq!(resets, 0);
    assert!(
        dma.registers()
            .iter()
            .any(|leaf| leaf.path() == "DmaRegisters.admin.channels[1].reset")
    );
    Ok(())
}
