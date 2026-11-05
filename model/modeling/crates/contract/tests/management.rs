//! Firmware control and host readiness register contracts.

use hw_model_schema::{Access, RegisterWindow};

#[test]
fn management_maps_define_distinct_control_and_status() -> Result<(), Box<dyn std::error::Error>> {
    // Given: the management and host-visible register maps.
    let control = pdma_model_contract::hw_components::windows::WindowControl::resolve()?;
    let status = pdma_model_contract::hw_components::windows::WindowStatus::resolve()?;
    // When: their software access metadata is resolved.
    let leaves = control
        .registers()
        .iter()
        .map(|leaf| {
            (
                leaf.offset(),
                leaf.definition().width(),
                leaf.definition().access(),
                leaf.definition().writable_mask(),
            )
        })
        .chain(status.registers().iter().map(|leaf| {
            (
                leaf.offset(),
                leaf.definition().width(),
                leaf.definition().access(),
                leaf.definition().writable_mask(),
            )
        }))
        .collect::<Vec<_>>();
    // Then: firmware controls bit zero, while host readiness is read-only.
    assert_eq!(
        leaves,
        [
            (0, 4, Access::ReadWrite, 1),
            (4, 4, Access::ReadWrite, 0xffff_ffff),
            (0, 4, Access::ReadOnly, 0)
        ]
    );
    Ok(())
}

#[test]
fn management_control_resets_closed_and_rejects_reserved_bits() {
    // Given: an ordinary generated window at the management base.
    let base = 0x2800;
    let mut control =
        RegisterWindow::<pdma_model_contract::hw_components::windows::WindowControl>::new(base);
    // When: a malformed enable attempts to change a reserved bit.
    let result = control.write(base, &3_u32.to_le_bytes());
    // Then: the generated access checks reject it without opening the window.
    assert!(result.is_err());
    assert!(!control.values.open.enabled());
}

#[test]
fn channel_window_offsets_are_relative_and_checked() {
    // Given: two channel selections and an overflowing selection.
    let first = pdma_model_contract::dma::ChannelId::new(0);
    let second = pdma_model_contract::dma::ChannelId::new(1);
    // When: resolving offsets independently of platform placement.
    let offsets = [
        first.window_control_offset(),
        first.window_status_offset(),
        second.window_control_offset(),
        second.window_status_offset(),
    ];
    // Then: complete compact blocks repeat without absolute address assumptions.
    assert_eq!(offsets, [Some(0), Some(8), Some(12), Some(20)]);
    assert_eq!(
        pdma_model_contract::dma::ChannelId::new(usize::MAX).window_status_offset(),
        None
    );
}

#[test]
fn repeated_window_metadata_matches_configured_count_and_role()
-> Result<(), Box<dyn std::error::Error>> {
    // Given: several hardware counts and both software roles.
    use pdma_model_contract::hw_components::windows::ChannelWindowRegisters;
    use pdma_model_contract::view::RegisterView;
    for count in [1, 2, 7] {
        let layout = ChannelWindowRegisters::new(count)?;
        for (role, access, first) in [
            (RegisterView::Host, Access::ReadOnly, 8),
            (RegisterView::Firmware, Access::ReadWrite, 0),
        ] {
            // When: filtering one complete physical bank for this role.
            let map = role.windows(&layout)?;
            // Then: each role retains whole blocks and its original leaf addresses.
            assert_window_metadata(&map, (count, layout.windows().stride(), first), access);
            assert_eq!(layout.windows().count(), count);
        }
    }
    Ok(())
}

fn assert_window_metadata<I: Copy>(
    map: &hw_model_schema::ResolvedRegisterMap<I>,
    (count, stride, first): (usize, usize, usize),
    access: Access,
) {
    assert_eq!(stride, 12);
    assert_eq!(map.size(), count * 12);
    let leaves = if access == Access::ReadWrite { 2 } else { 1 };
    assert_eq!(map.registers().len(), count * leaves);
    for (index, leaf) in map.registers().iter().enumerate() {
        assert_eq!(
            leaf.offset(),
            (index / leaves) * 12 + first + (index % leaves) * 4
        );
        assert_eq!(leaf.definition().access(), access);
        assert_eq!(leaf.definition().width(), 4);
    }
}
