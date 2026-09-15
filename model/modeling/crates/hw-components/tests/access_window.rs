//! Access permission boundaries against a real downstream register map.

use hardware_interface_schema::{RegisterAccessError, RegisterWindow, register_map};
use pdma_model_contract::{WINDOW_CONTROL_BASE, WINDOW_STATUS_BASE, Word};

type TestResult = Result<(), Box<dyn std::error::Error>>;

register_map! {
    map Downstream {
        alignment: 4,
        placements: [pub data: Word, pub sibling: Word],
    }
}

#[test]
fn closed_window_preserves_downstream_and_output() -> TestResult {
    // Given: closed hardware in front of ordinary register storage.
    let window = pdma_model_hw_components::AccessWindow::new(8)?;
    let mut registers = RegisterWindow::<Downstream>::new(0);
    registers.write(0, &17_u32.to_le_bytes())?;
    let mut output = [0xa5; 4];
    // When: host attempts reads and writes before firmware opens access.
    let write = window.write(0, &42_u32.to_le_bytes(), |address, bytes| {
        registers.write(address, bytes)
    });
    let read = window.read(0, &mut output, |address, bytes| {
        registers.read(address, bytes)
    });
    // Then: both are blocked and neither hardware storage nor host bytes change.
    assert_eq!(write, Err(pdma_model_hw_components::AccessError::Closed));
    assert_eq!(read, Err(pdma_model_hw_components::AccessError::Closed));
    assert_eq!(registers.values.data.raw(), 17);
    assert_eq!(output, [0xa5; 4]);
    Ok(())
}

#[test]
fn open_window_forwards_real_register_access() -> TestResult {
    // Given: firmware opens a window containing two register leaves.
    let mut window = pdma_model_hw_components::AccessWindow::new(8)?;
    window.management_write(usize::try_from(WINDOW_CONTROL_BASE)?, &1_u32.to_le_bytes())?;
    let mut registers = RegisterWindow::<Downstream>::new(0);
    registers.write(0, &17_u32.to_le_bytes())?;
    let mut output = [0; 4];
    // When: host writes the second leaf and reads it back.
    window.write(4, &42_u32.to_le_bytes(), |address, bytes| {
        registers.write(address, bytes)
    })?;
    window.read(4, &mut output, |address, bytes| {
        registers.read(address, bytes)
    })?;
    // Then: byte addresses and values reach the selected downstream leaf.
    assert_eq!(output, 42_u32.to_le_bytes());
    assert_eq!(registers.values.data.raw(), 17);
    Ok(())
}

#[test]
fn readiness_tracks_firmware_control_without_host_authority() -> TestResult {
    // Given: the same real downstream window for both access states.
    let mut window = pdma_model_hw_components::AccessWindow::new(8)?;
    let mut registers = RegisterWindow::<Downstream>::new(0);
    let control = usize::try_from(WINDOW_CONTROL_BASE)?;
    let status = usize::try_from(WINDOW_STATUS_BASE)?;
    for ready in [0_u32, 1, 0] {
        window.management_write(control, &ready.to_le_bytes())?;
        let mut output = [0xa5; 4];
        // When: host observes status and attempts management operations.
        window.read(status, &mut output, |address, bytes| {
            registers.read(address, bytes)
        })?;
        let denied_read = window.read(control, &mut [0; 4], |address, bytes| {
            registers.read(address, bytes)
        });
        let denied_control = window.write(control, &(1 - ready).to_le_bytes(), |address, bytes| {
            registers.write(address, bytes)
        });
        let denied_status = window.write(status, &(1 - ready).to_le_bytes(), |address, bytes| {
            registers.write(address, bytes)
        });
        // Then: host sees the firmware bit but cannot change it through either map.
        assert_eq!(output, ready.to_le_bytes());
        assert_eq!(
            denied_read,
            Err(pdma_model_hw_components::AccessError::Denied)
        );
        assert_eq!(
            denied_control,
            Err(pdma_model_hw_components::AccessError::Denied)
        );
        assert_eq!(
            denied_status,
            Err(pdma_model_hw_components::AccessError::Denied)
        );
        assert_eq!(window.is_open(), ready == 1);
    }
    Ok(())
}

#[test]
fn malformed_downstream_access_retains_its_original_error() -> TestResult {
    // Given: open access to real generated register validation.
    let mut window = pdma_model_hw_components::AccessWindow::new(8)?;
    window.management_write(usize::try_from(WINDOW_CONTROL_BASE)?, &1_u32.to_le_bytes())?;
    let mut registers = RegisterWindow::<Downstream>::new(0);
    // When: host writes fewer bytes than the selected leaf requires.
    let result = window.write(0, &[1], |address, bytes| registers.write(address, bytes));
    // Then: hardware retains the downstream error without changing storage.
    assert_eq!(
        result,
        Err(pdma_model_hw_components::AccessError::Downstream(
            RegisterAccessError::WidthMismatch {
                offset: 0,
                expected: 4,
                actual: 1
            }
        ))
    );
    assert_eq!(registers.values.data.raw(), 0);
    Ok(())
}
