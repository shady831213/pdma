//! Both software views derive from one repeated permission block.

use hw_model_schema::Access;
use pdma_model_contract::hw_components::windows::ChannelWindowRegisters;
use pdma_model_contract::view::RegisterView;

#[test]
fn role_views_preserve_the_complete_channel_window_geometry()
-> Result<(), Box<dyn std::error::Error>> {
    // Given: a bank containing three complete channel permission blocks.
    let layout = ChannelWindowRegisters::new(3)?;
    // When: each software role selects its accessible registers.
    let host = RegisterView::Host.windows(&layout)?;
    let firmware = RegisterView::Firmware.windows(&layout)?;
    // Then: both views retain the same bank while exposing disjoint leaves.
    assert_eq!(
        (layout.extent(), host.size(), firmware.size()),
        (36, 36, 36)
    );
    for (map, offsets, access) in [
        (&host, vec![8, 20, 32], Access::ReadOnly),
        (&firmware, vec![0, 4, 12, 16, 24, 28], Access::ReadWrite),
    ] {
        assert_eq!(
            map.registers()
                .iter()
                .map(hw_model_schema::ResolvedRegister::offset)
                .collect::<Vec<_>>(),
            offsets
        );
        assert!(
            map.registers()
                .iter()
                .all(|r| r.definition().access() == access)
        );
        assert_eq!(map.banks().len(), 1);
        let bank = map.banks().first().unwrap();
        assert_eq!((bank.base(), bank.count(), bank.stride()), (0, 3, 12));
    }
    Ok(())
}
