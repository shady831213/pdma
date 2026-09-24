use hardware_interface_schema::Access;
use pdma_model_contract::hw_components::control::ControlRegisters;
use pdma_model_contract::memory::FunctionId;
use pdma_model_contract::view::RegisterView;
use pdma_model_hw_components::control::ControlChannel;

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
            let mut device = ControlChannel::new(0, FunctionId::new(0));
            // When: the selected domain issues full-width reads and writes.
            let (read, write) = match role {
                RegisterView::Host => (
                    device
                        .host_read(FunctionId::new(0), offset, &mut bytes)
                        .is_ok(),
                    device
                        .host_write(FunctionId::new(0), offset, &bytes)
                        .is_ok(),
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
