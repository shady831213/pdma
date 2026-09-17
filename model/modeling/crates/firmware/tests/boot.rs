//! Firmware boot failures at the software CPU and management transport boundaries.

use hardware_interface_runtime::{CpuContextError, NotificationWait, RegisterAccess, SoftwareCpu};
use std::{convert::Infallible, io};

#[derive(Debug)]
struct FailedBus;

impl NotificationWait for FailedBus {
    async fn wait_notification(&self) {
        std::future::pending::<()>().await;
    }
}

impl RegisterAccess for FailedBus {
    type Error = io::Error;

    async fn read(&self, _: u32, _: &mut [u8]) -> Result<(), Self::Error> {
        Err(io::Error::other("management transport failed"))
    }

    async fn write(&self, _: u32, _: &[u8]) -> Result<(), Self::Error> {
        Err(io::Error::other("management transport failed"))
    }
}

#[tokio::test]
async fn boot_reports_management_failure() {
    // Given: a software CPU and an unavailable management transport.
    let cpu = SoftwareCpu::new();
    // When: firmware attempts initialization.
    let result = cpu
        .run_main(pdma_model_firmware::main(
            FailedBus,
            hardware_interface_runtime::SharedMemory::new(),
        ))
        .await;
    // Then: transport failure reaches the supervisor instead of reporting readiness.
    assert!(matches!(result, Err(pdma_model_firmware::Error::Bus(_))));
}

#[tokio::test]
async fn boot_requires_a_software_cpu() {
    // Given: firmware is called without a modeled CPU execution domain.
    // When: the program starts.
    let result: Result<Infallible, _> =
        pdma_model_firmware::main(FailedBus, hardware_interface_runtime::SharedMemory::new()).await;
    // Then: it cannot perform management accesses as unowned runtime code.
    assert!(matches!(
        result,
        Err(pdma_model_firmware::Error::Cpu(CpuContextError::Inactive))
    ));
}
