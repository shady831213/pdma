//! Register metadata follows each hardware instance's channel count.

use pdma_model_contract::{ChannelId, DeviceRegisters};

#[test]
fn resolved_channels_follow_each_instance_count() -> Result<(), Box<dyn std::error::Error>> {
    // Given: distinct instance configurations using the same contract type.
    for (count, extent) in [(1, 84), (3, 228), (5, 372)] {
        // When: resolving that instance's generated child blocks.
        let map = DeviceRegisters::new(count)?.resolve()?;
        // Then: only its configured channels occur, at the established byte geometry.
        assert_eq!(map.size(), extent);
        assert_eq!(map.registers().len(), 4 + 14 * count);
        for index in 0..count {
            let base = 16 + 72 * index;
            assert_eq!(ChannelId::new(index).register_offset(), Some(base));
            assert_eq!(
                map.locate(base + 36).expect("response producer").offset(),
                base + 36
            );
        }
        assert!(map.locate(16 + 72 * count).is_none());
    }
    Ok(())
}

#[test]
fn invalid_instance_geometry_is_rejected_before_expansion() {
    // Given: empty and overflowing instance channel counts.
    let counts = [0, usize::MAX];
    // When: validating extent without allocating channel metadata.
    let results = counts.map(DeviceRegisters::new);
    // Then: neither geometry is representable by a hardware instance.
    assert!(results.into_iter().all(|result| result.is_err()));
}
