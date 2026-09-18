//! Boot status reset and access boundaries.

use hardware_interface_schema::RegisterAccessError;
use pdma_model_contract::{FirmwareBootError, FirmwareBootState, FirmwareBootValues};
use pdma_model_hw_components::{AccessWindow, FirmwareStatus};

#[test]
fn reset_discards_ready_and_failure_reports() {
    // Given: firmware has previously published either completion outcome.
    for raw in [1_u32, 2 | (1 << 16)] {
        let mut status = FirmwareStatus::new(0x100);
        status.management_write(0x100, &raw.to_le_bytes()).unwrap();
        // When: the hardware status is reset.
        status.reset();
        // Then: Host sees Booting with no stale error.
        let mut bytes = [0xff; 4];
        status.read(0x100, &mut bytes).unwrap();
        let value = FirmwareBootValues::new(u32::from_le_bytes(bytes));
        assert_eq!(value.state().unwrap(), FirmwareBootState::Booting);
        assert_eq!(value.error().unwrap(), FirmwareBootError::None);
    }
}

#[test]
fn ready_does_not_open_the_dma_window() {
    // Given: an independent closed DMA gate and boot-status register.
    let window = AccessWindow::new(0..16, 0x0010_0000).unwrap();
    let mut status = FirmwareStatus::new(0x100);
    // When: firmware publishes Ready.
    status
        .management_write(0x100, &1_u32.to_le_bytes())
        .unwrap();
    // Then: readiness is visible without granting DMA access.
    let mut bytes = [0; 4];
    status.read(0x100, &mut bytes).unwrap();
    assert_eq!(bytes, 1_u32.to_le_bytes());
    assert!(!window.is_open());
}

#[test]
fn host_cannot_publish_firmware_status() {
    // Given: the Host view of a reset status register.
    let mut status = FirmwareStatus::new(0x100);
    // When: Host attempts to forge Ready.
    let result = status.host_write(0x100, &1_u32.to_le_bytes());
    // Then: the write is rejected and Booting remains visible.
    assert!(matches!(
        result,
        Err(RegisterAccessError::WriteForbidden { .. })
    ));
    let mut bytes = [0xff; 4];
    status.read(0x100, &mut bytes).unwrap();
    assert_eq!(bytes, [0; 4]);
}

#[test]
fn malformed_status_access_preserves_state_and_output() {
    // Given: a reset boot status.
    let mut status = FirmwareStatus::new(0x100);
    // When: accesses use an unknown address, wrong width or reserved bit.
    assert!(matches!(
        status.management_write(0x104, &[0; 4]),
        Err(RegisterAccessError::OutsideWindow { .. })
    ));
    assert!(status.management_write(0x100, &[1; 3]).is_err());
    assert!(
        status
            .management_write(0x100, &4_u32.to_le_bytes())
            .is_err()
    );
    let mut output = [0xa5; 3];
    assert!(status.read(0x100, &mut output).is_err());
    // Then: no failed access changed status or the destination buffer.
    assert_eq!(output, [0xa5; 3]);
    let mut bytes = [0xff; 4];
    status.read(0x100, &mut bytes).unwrap();
    assert_eq!(bytes, [0; 4]);
}
