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
    assert_eq!(pdma_model_contract::WINDOW_CONTROL_BASE, 0xffff_ff00);
    assert_eq!(pdma_model_contract::WINDOW_STATUS_BASE, 0xffff_ff08);
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
