use pdma_model_hw_components::windows::ChannelWindow;
const WINDOW_CONTROL_BASE: u32 = 0x0010_0000;
const WINDOW_STATUS_BASE: u32 = 0x0010_0008;

#[test]
fn window_views_match_the_two_hardware_entry_points() {
    // Given: a closed access window with management control and Host readiness.
    let mut window =
        ChannelWindow::new(0..256, usize::try_from(WINDOW_CONTROL_BASE).unwrap()).unwrap();
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
