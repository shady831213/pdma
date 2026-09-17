//! Instance layouts preserve hardware geometry without owning register values.

use pdma_model_contract::DeviceRegisters;

#[test]
fn instance_bank_matches_resolved_register_addresses() -> Result<(), Box<dyn std::error::Error>> {
    // Given: the same schema instantiated with several immutable channel counts.
    for (count, extent) in [(1, 84), (3, 228), (5, 372)] {
        let layout = DeviceRegisters::new(count)?;
        let bank = layout.channels();
        // When: indexed bank geometry and resolved metadata describe the instance.
        let resolved = layout.resolve()?;
        // Then: both select exactly the existing hardware addresses, including a one-element bank.
        assert_eq!(layout.extent(), extent);
        assert_eq!(resolved.size(), extent);
        assert_eq!((bank.base(), bank.count(), bank.stride()), (16, count, 72));
        for index in 0..count {
            let address = 16 + index * 72;
            assert_eq!(bank.offset(index), Some(address));
            assert_eq!(
                resolved.locate(address + 36).unwrap().offset(),
                address + 36
            );
        }
        assert_eq!(bank.offset(count), None);
    }
    Ok(())
}
