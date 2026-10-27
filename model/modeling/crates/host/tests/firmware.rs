//! Host boot polling outcomes.
const FIRMWARE_STATUS_BASE: u32 = 0x4000;

use hardware_interface_io::register::RegisterAccess;
use hardware_interface_software::SoftwareEnv;
use pdma_model_host::{Error, firmware};
use std::{io, time::Duration};

struct StatusBus(u32);

impl RegisterAccess for StatusBus {
    type Error = io::Error;
    async fn read(&self, address: u32, bytes: &mut [u8]) -> Result<(), Self::Error> {
        assert_eq!(address, FIRMWARE_STATUS_BASE);
        bytes.copy_from_slice(&self.0.to_le_bytes());
        Ok(())
    }
    async fn write(&self, _: u32, _: &[u8]) -> Result<(), Self::Error> {
        Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "read-only status bus",
        ))
    }
}

#[tokio::test]
async fn ready_allows_host_to_continue() {
    // Given: firmware has published Ready.
    let bus = StatusBus(1);
    // When: Host waits through its owning CPU.
    let result = SoftwareEnv::new()
        .run_main(firmware::wait_ready(
            &bus,
            FIRMWARE_STATUS_BASE,
            Duration::from_secs(1),
        ))
        .await;
    // Then: it can proceed without modifying hardware.
    assert!(result.is_ok());
}

#[tokio::test]
async fn failed_reports_the_firmware_error_code() {
    // Given: firmware reports a window initialization failure.
    let bus = StatusBus(2 | (1 << 16));
    // When: Host waits for readiness.
    let result = SoftwareEnv::new()
        .run_main(firmware::wait_ready(
            &bus,
            FIRMWARE_STATUS_BASE,
            Duration::from_secs(1),
        ))
        .await;
    // Then: the reported failure is not confused with a timeout.
    assert!(matches!(
        result,
        Err(Error::FirmwareFailed(
            pdma_model_contract::hw_components::firmware::FirmwareBootError::WindowSetup
        ))
    ));
}

#[tokio::test(start_paused = true)]
async fn booting_does_not_mean_ready_and_host_can_time_out() {
    // Given: firmware never leaves Booting.
    let bus = StatusBus(0);
    // When: the Host's deadline expires, using virtual time.
    let waiting = async {
        SoftwareEnv::new()
            .run_main(firmware::wait_ready(
                &bus,
                FIRMWARE_STATUS_BASE,
                Duration::from_secs(1),
            ))
            .await
    };
    let clock = async {
        tokio::task::yield_now().await;
        tokio::time::advance(Duration::from_secs(2)).await;
    };
    let (result, ()) = tokio::join!(waiting, clock);
    // Then: software receives a timeout, not a fabricated firmware failure.
    assert!(matches!(result, Err(Error::FirmwareTimeout)));
}

#[tokio::test]
async fn invalid_firmware_state_is_rejected() {
    // Given: an invalid state encoding from the register boundary.
    let bus = StatusBus(3);
    // When: Host decodes the report.
    let result = SoftwareEnv::new()
        .run_main(firmware::wait_ready(
            &bus,
            FIRMWARE_STATUS_BASE,
            Duration::from_secs(1),
        ))
        .await;
    // Then: invalid data cannot be interpreted as Ready.
    assert!(matches!(result, Err(Error::RegisterCodec(_))));
}
