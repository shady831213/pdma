//! Register metadata follows each hardware instance's channel count.

use pdma_model_contract::dma::{ChannelId, DmaRegisters};

#[test]
fn resolved_channels_follow_each_instance_count() -> Result<(), Box<dyn std::error::Error>> {
    // Given: distinct instance configurations using the same contract type.
    for (count, extent) in [(1, 8192), (3, 16384), (5, 24576)] {
        // When: resolving that instance's generated child blocks.
        let map = DmaRegisters::new(count)?.resolve()?;
        // Then: only its configured channels occur, at the established byte geometry.
        assert_eq!(map.size(), extent);
        assert_eq!(map.registers().len(), 4 + 17 * count);
        for index in 0..count {
            let base = 4096 + 4096 * index;
            assert_eq!(ChannelId::new(index).register_offset(count), Some(base));
            assert_eq!(
                map.locate(base + 36).expect("response producer").offset(),
                base + 36
            );
        }
        assert!(map.locate(extent).is_none());
    }
    Ok(())
}

#[test]
fn invalid_instance_geometry_is_rejected_before_expansion() {
    // Given: empty and overflowing instance channel counts.
    let counts = [0, usize::MAX];
    // When: validating extent without allocating channel metadata.
    let results = counts.map(DmaRegisters::new);
    // Then: neither geometry is representable by a hardware instance.
    assert!(results.into_iter().all(|result| result.is_err()));
}

#[test]
fn growing_admin_bank_moves_data_to_the_next_complete_page() {
    // Given: the last channel count whose administration fits in the first page.
    let before = DmaRegisters::new(1020).unwrap();
    let after = DmaRegisters::new(1021).unwrap();
    // When: another channel grows administration across that page boundary.
    let starts = [before.channels().base(), after.channels().base()];
    // Then: data remains page-aligned and never overlaps any administrative register.
    assert_eq!(starts, [4096, 8192]);
    assert_eq!(before.admin().end(), 4096);
    assert_eq!(after.admin().end(), 4100);
    assert_eq!(ChannelId::new(0).register_offset(1021), Some(8192));
    assert_eq!(ChannelId::new(1020).admin_offset(1021), Some(4096));
    assert_eq!(ChannelId::new(1021).register_offset(1021), None);
}
