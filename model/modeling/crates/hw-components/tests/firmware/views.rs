use hw_model_schema::Access;
use pdma_model_contract::hw_components::firmware::FirmwareRegisters;
use pdma_model_contract::view::RegisterView;
use pdma_model_hw_components::firmware::FirmwareStatus;

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
