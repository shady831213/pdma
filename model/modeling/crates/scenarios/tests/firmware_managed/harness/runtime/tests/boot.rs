use super::*;
use hardware_interface_runtime::SoftwareCpu;
use pdma_model_contract::FIRMWARE_STATUS_BASE;
use pdma_model_host::{Error, firmware};
use std::{pin::pin, task::Poll, time::Duration};

struct RejectWindow<B>(B);

impl<B: RegisterAccess<Error = AccessError> + Sync> RegisterAccess for RejectWindow<B> {
    type Error = AccessError;
    async fn read(&self, address: u32, bytes: &mut [u8]) -> Result<(), AccessError> {
        self.0.read(address, bytes).await
    }
    async fn write(&self, address: u32, bytes: &[u8]) -> Result<(), AccessError> {
        if address == WINDOW_CONTROL_BASE {
            return Err(AccessError::Denied);
        }
        self.0.write(address, bytes).await
    }
}

#[tokio::test]
async fn boot_state_is_visible_before_firmware_starts() {
    // Given: the subsystem is assembled in its reset state.
    let (_system, host) =
        Managed::new(SharedMemoryAccess::from(Arc::new(SharedMemory::new()))).unwrap();
    // When: Host reads the always-on firmware status.
    let mut word = [0xff; 4];
    host.bus
        .read(FIRMWARE_STATUS_BASE, &mut word)
        .await
        .unwrap();
    // Then: it observes Booting without access to DMA or firmware write permissions.
    assert_eq!(word, [0; 4]);
    assert!(matches!(
        host.bus.read(0, &mut word).await,
        Err(AccessError::Closed)
    ));
    assert!(
        host.bus
            .write(FIRMWARE_STATUS_BASE, &1u32.to_le_bytes())
            .await
            .is_err()
    );
}

#[tokio::test]
async fn firmware_reports_initialization_failure_to_host_through_hardware() {
    // Given: a real status register and a rejected window setup transaction.
    let (system, host) =
        Managed::new(SharedMemoryAccess::from(Arc::new(SharedMemory::new()))).unwrap();
    let bus = RejectWindow(super::super::ManagementBus(Arc::clone(&system.shared)));
    let bus = system.firmware_cpu.bind_registers(bus);
    let mut running = pin!(system.firmware_runtime.run_main(async |registration| {
        pdma_model_firmware::main(bus, system.firmware_memory, registration).await
    }));
    // When: firmware initializes on its own CPU.
    let outcome = std::future::poll_fn(|cx| Poll::Ready(running.as_mut().poll(cx))).await;
    // Then: it stays quiescent and Host reads its reported failure, not a supervisor error.
    assert!(outcome.is_pending());
    let result = SoftwareCpu::new()
        .run_main(firmware::wait_ready(&host.bus, Duration::from_secs(1)))
        .await;
    assert!(matches!(
        result,
        Err(Error::FirmwareFailed(
            pdma_model_contract::FirmwareBootError::WindowSetup
        ))
    ));
    let mut word = [0; 4];
    host.bus.read(WINDOW_STATUS_BASE, &mut word).await.unwrap();
    assert_eq!(word, [0; 4]);
    assert!(matches!(
        host.bus.read(0, &mut word).await,
        Err(AccessError::Closed)
    ));
}
