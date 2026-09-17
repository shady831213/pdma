//! Both software views derive from one repeated permission block.

use hardware_interface_schema::Access;
use pdma_model_contract::{ChannelWindowRegisters, RegisterView};

#[test]
fn role_views_preserve_the_complete_channel_window_geometry()
-> Result<(), Box<dyn std::error::Error>> {
    // Given: a bank containing three complete channel permission blocks.
    let layout = ChannelWindowRegisters::new(3)?;
    // When: each software role selects its accessible registers.
    let full = layout.resolve()?;
    let project = |role: RegisterView| {
        full.project_access(|register| {
            let (_, offset) = layout.windows().locate_instance(register.offset())?;
            role.window_access(offset)
        })
    };
    let host = project(RegisterView::Host)?;
    let firmware = project(RegisterView::Firmware)?;
    // Then: both views retain the same bank while exposing disjoint leaves.
    assert_eq!(
        (layout.extent(), host.size(), firmware.size()),
        (48, 48, 48)
    );
    for (map, offsets, access) in [
        (&host, [8, 24, 40], Access::ReadOnly),
        (&firmware, [0, 16, 32], Access::ReadWrite),
    ] {
        assert_eq!(
            map.registers()
                .iter()
                .map(hardware_interface_schema::ResolvedRegister::offset)
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
        assert_eq!((bank.base(), bank.count(), bank.stride()), (0, 3, 16));
    }
    Ok(())
}
