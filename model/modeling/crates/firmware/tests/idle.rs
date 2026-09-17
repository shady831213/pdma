//! Firmware suspension before Host activates its control queue.

use hardware_interface_runtime::{NotificationWait, RegisterAccess, SharedMemory, SoftwareCpu};
use std::{
    future::Future,
    io,
    sync::atomic::{AtomicUsize, Ordering},
    task::{Context, Waker},
};

#[derive(Default)]
struct DisabledBus(AtomicUsize);

impl RegisterAccess for &DisabledBus {
    type Error = io::Error;
    async fn read(&self, _: u32, bytes: &mut [u8]) -> Result<(), Self::Error> {
        self.0.fetch_add(1, Ordering::SeqCst);
        bytes.fill(0);
        Ok(())
    }
    async fn write(&self, _: u32, _: &[u8]) -> Result<(), Self::Error> {
        Ok(())
    }
}

impl NotificationWait for &DisabledBus {
    async fn wait_notification(&self) {
        std::future::pending::<()>().await;
    }
}

#[tokio::test]
async fn disabled_control_waits_without_repeated_register_reads() {
    // Given: boot succeeds but Host has not activated the queue.
    let bus = DisabledBus::default();
    let cpu = SoftwareCpu::new();
    let mut program =
        std::pin::pin!(cpu.run_main(pdma_model_firmware::main(&bus, SharedMemory::new())));
    let mut context = Context::from_waker(Waker::noop());
    // When: the executor polls the idle program repeatedly without a doorbell.
    for _ in 0..8 {
        assert!(program.as_mut().poll(&mut context).is_pending());
    }
    // Then: activation was read once and firmware remains suspended.
    assert_eq!(bus.0.load(Ordering::SeqCst), 1);
}
