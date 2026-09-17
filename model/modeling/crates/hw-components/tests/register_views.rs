//! Static contract views match hardware domain permissions.

use hardware_interface_schema::Access;
use pdma_model_contract::{
    ControlRegisters, FirmwareRegisters, RegisterView, WINDOW_CONTROL_BASE, WINDOW_STATUS_BASE,
};
use pdma_model_hw_components::{AccessWindow, ControlChannel, FirmwareStatus};

#[test]
fn control_view_permissions_match_real_hardware_transactions() {
    // Given: each physical control leaf and a fresh disabled control channel.
    let original = ControlRegisters::resolve().unwrap();
    for role in [RegisterView::Host, RegisterView::Firmware] {
        let view = original
            .project_access(|r| role.control_access(r.offset()))
            .unwrap();
        assert_eq!(view.size(), original.size());
        for register in original.registers() {
            let offset = register.offset();
            let mut bytes = vec![0; register.definition().width()];
            let mut device = ControlChannel::new(0);
            // When: the selected domain issues full-width reads and writes.
            let (read, write) = match role {
                RegisterView::Host => (
                    device.host_read(offset, &mut bytes).is_ok(),
                    device.host_write(offset, &bytes).is_ok(),
                ),
                RegisterView::Firmware => (
                    device.firmware_read(offset, &mut bytes).is_ok(),
                    device.firmware_write(offset, &bytes).is_ok(),
                ),
            };
            // Then: the projected metadata exactly describes hardware permission.
            let access = view.locate(offset).map(|leaf| leaf.definition().access());
            assert_eq!(
                (read, write),
                match access {
                    None => (false, false),
                    Some(Access::ReadOnly) => (true, false),
                    Some(Access::WriteOnly) => (false, true),
                    Some(Access::ReadWrite) => (true, true),
                },
                "{role:?} at {offset}"
            );
        }
    }
}

#[test]
fn boot_view_permissions_match_real_hardware_transactions() {
    // Given: the firmware boot register with each role's static projection.
    for role in [RegisterView::Host, RegisterView::Firmware] {
        let view = FirmwareRegisters::resolve()
            .unwrap()
            .project_access(|_| Some(role.boot_access()))
            .unwrap();
        let mut device = FirmwareStatus::new(0);
        let mut bytes = [0; 4];
        // When: each role reads then attempts to publish a boot report.
        let read = device.read(0, &mut bytes).is_ok();
        let write = match role {
            RegisterView::Host => device.host_write(0, &bytes).is_ok(),
            RegisterView::Firmware => device.management_write(0, &bytes).is_ok(),
        };
        // Then: both read, but only firmware receives writable metadata and permission.
        assert!(read);
        let definition = view.locate(0).unwrap().definition();
        assert_eq!(write, definition.access() == Access::ReadWrite);
        assert_eq!(definition.writable_mask() != 0, write);
    }
}

#[test]
fn window_views_match_the_two_hardware_entry_points() {
    // Given: a closed access window with management control and Host readiness.
    let mut window = AccessWindow::new(256).unwrap();
    let control = usize::try_from(WINDOW_CONTROL_BASE).unwrap();
    let status = usize::try_from(WINDOW_STATUS_BASE).unwrap();
    let mut bytes = [0; 4];
    // When: legal-width transactions exercise both entry points.
    // Then: Host only reads readiness, firmware accesses control, and the work gate stays closed.
    assert!(
        window
            .read(status, &mut bytes, |_, _| Ok::<_, ()>(()))
            .is_ok()
    );
    assert!(
        window
            .write(status, &bytes, |_, _| Ok::<_, ()>(()))
            .is_err()
    );
    assert!(
        window
            .read(control, &mut bytes, |_, _| Ok::<_, ()>(()))
            .is_err()
    );
    assert!(
        window
            .write(control, &bytes, |_, _| Ok::<_, ()>(()))
            .is_err()
    );
    assert!(window.management_read(control, &mut bytes).is_ok());
    assert!(window.management_write(control, &bytes).is_ok());
    assert!(window.management_read(status, &mut bytes).is_err());
    assert!(window.management_write(status, &bytes).is_err());
    assert!(window.read(0, &mut bytes, |_, _| Ok::<_, ()>(())).is_err());
}
