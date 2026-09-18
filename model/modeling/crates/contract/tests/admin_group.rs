//! Administration must precede channel data apertures.

use pdma_model_contract::DmaRegisters;

#[test]
fn all_channel_administration_precedes_data() {
    // Given: several independently configured DMA populations.
    for count in [1, 3, 5] {
        let layout = DmaRegisters::new(count).unwrap();
        // When: comparing complete administration and data bank envelopes.
        let admin_end = layout.admin().end();
        // Then: privileged channel recovery precedes every data aperture.
        assert!(admin_end <= layout.channels().base());
    }
}
