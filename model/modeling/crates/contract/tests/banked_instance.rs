//! Instance layouts preserve hardware geometry without owning register values.

use pdma_model_contract::dma::{ChannelId, DmaRegisters};

#[test]
fn instance_bank_matches_resolved_register_addresses() -> Result<(), Box<dyn std::error::Error>> {
    // Given: the same schema instantiated with several immutable channel counts.
    for (count, extent) in [(1, 8192), (3, 16384), (5, 24576)] {
        let layout = DmaRegisters::new(count)?;
        let bank = layout.channels();
        // When: indexed bank geometry and resolved metadata describe the instance.
        let resolved = layout.resolve()?;
        // Then: both select exactly the existing hardware addresses, including a one-element bank.
        assert_eq!(layout.extent(), extent);
        assert_eq!(resolved.size(), extent);
        assert_eq!(
            (bank.base(), bank.count(), bank.stride()),
            (4096, count, 4096)
        );
        for index in 0..count {
            let address = 4096 + index * 4096;
            assert_eq!(bank.offset(index), Some(address));
            assert_eq!(
                ChannelId::new(index).admin_offset(count),
                Some(20 + 4 * index)
            );
            assert_eq!(
                resolved.locate(address + 36).unwrap().offset(),
                address + 36
            );
        }
        assert_eq!(bank.offset(count), None);
        assert_eq!(ChannelId::new(count).admin_offset(count), None);
    }
    Ok(())
}
