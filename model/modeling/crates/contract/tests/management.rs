//! Firmware control and host readiness register contracts.

use hardware_interface_schema::{Access, RegisterWindow};

#[test]
fn management_maps_define_distinct_control_and_status() -> Result<(), Box<dyn std::error::Error>> {
    // Given: the management and host-visible register maps.
    let control = pdma_model_contract::WindowControl::resolve()?;
    let status = pdma_model_contract::WindowStatus::resolve()?;
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
    assert_eq!(pdma_model_contract::WINDOW_CONTROL_BASE, 0xff00_0000);
    assert_eq!(pdma_model_contract::WINDOW_STATUS_BASE, 0xff00_0008);
    assert_eq!(
        leaves,
        [(0, 4, Access::ReadWrite, 1), (0, 4, Access::ReadOnly, 0)]
    );
    Ok(())
}

#[test]
fn management_control_resets_closed_and_rejects_reserved_bits()
-> Result<(), Box<dyn std::error::Error>> {
    // Given: an ordinary generated window at the management base.
    let base = usize::try_from(pdma_model_contract::WINDOW_CONTROL_BASE)?;
    let mut control = RegisterWindow::<pdma_model_contract::WindowControl>::new(base);
    // When: a malformed enable attempts to change a reserved bit.
    let result = control.write(base, &3_u32.to_le_bytes());
    // Then: the generated access checks reject it without opening the window.
    assert!(result.is_err());
    assert!(!control.values.open.enabled());
    Ok(())
}

#[test]
fn channel_window_addresses_stop_before_always_on_control() {
    // Given: the first, last admissible and first overlapping selections.
    let count =
        (pdma_model_contract::CONTROL_CHANNEL_BASE - pdma_model_contract::WINDOW_CONTROL_BASE) / 16;
    // When: channel selections resolve their management addresses.
    let first = pdma_model_contract::ChannelId::new(0);
    let last = pdma_model_contract::ChannelId::new(usize::try_from(count - 1).unwrap());
    let outside = pdma_model_contract::ChannelId::new(usize::try_from(count).unwrap());
    // Then: every complete slot fits below always-on control, with overflow rejected.
    assert_eq!(first.window_control_address(), Some(0xff00_0000));
    assert_eq!(first.window_status_address(), Some(0xff00_0008));
    assert_eq!(
        last.window_status_address(),
        Some(pdma_model_contract::CONTROL_CHANNEL_BASE - 8)
    );
    assert_eq!(outside.window_control_address(), None);
    assert_eq!(
        pdma_model_contract::ChannelId::new(usize::MAX).window_status_address(),
        None
    );
}

#[test]
fn repeated_window_metadata_matches_configured_count_and_role()
-> Result<(), Box<dyn std::error::Error>> {
    // Given: several hardware counts and both software roles.
    use pdma_model_contract::{ChannelWindowRegisters, RegisterView};
    for count in [1, 2, 7] {
        let layout = ChannelWindowRegisters::new(count)?;
        for (role, access, first) in [
            (RegisterView::Host, Access::ReadOnly, 8),
            (RegisterView::Firmware, Access::ReadWrite, 0),
        ] {
            // When: filtering one complete physical bank for this role.
            let map = layout.resolve()?.project_access(|register| {
                let (_, offset) = layout.windows().locate_instance(register.offset())?;
                role.window_access(offset)
            })?;
            // Then: each role retains whole blocks and its original leaf addresses.
            assert_window_metadata(&map, (count, layout.windows().stride(), first), access);
            assert_eq!(layout.windows().count(), count);
        }
    }
    Ok(())
}

fn assert_window_metadata<I: Copy>(
    map: &hardware_interface_schema::ResolvedRegisterMap<I>,
    (count, stride, first): (usize, usize, usize),
    access: Access,
) {
    assert_eq!(stride, 16);
    assert_eq!(map.size(), count * 16);
    assert_eq!(map.registers().len(), count);
    for (index, leaf) in map.registers().iter().enumerate() {
        assert_eq!(leaf.offset(), index * 16 + first);
        assert_eq!(leaf.definition().access(), access);
        assert_eq!(leaf.definition().width(), 4);
    }
}
