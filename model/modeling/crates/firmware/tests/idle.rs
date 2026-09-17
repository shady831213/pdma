//! Firmware suspension before Host activates its control queue.

use hardware_interface_runtime::{
    InterruptRuntime, RegisterAccess, SharedMemory, SoftwareCpu, pending_interrupt,
};
use pdma_model_software::control::{ControlIrq, ControlIsr};
use std::{
    future::Future,
    io,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    task::{Context, Waker},
};

#[derive(Default)]
struct DisabledBus(AtomicUsize);

impl RegisterAccess for &DisabledBus {
    type Error = io::Error;
    async fn read(&self, address: u32, bytes: &mut [u8]) -> Result<(), Self::Error> {
        bytes.fill(0);
        if address == 0 {
            bytes.copy_from_slice(&1_u32.to_le_bytes());
        } else if address == pdma_model_contract::CONTROL_CHANNEL_BASE + 8 {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
        Ok(())
    }
    async fn write(&self, _: u32, _: &[u8]) -> Result<(), Self::Error> {
        Ok(())
    }
}

#[tokio::test]
async fn disabled_control_waits_without_repeated_register_reads() {
    // Given: boot succeeds but Host has not activated the queue.
    let bus = DisabledBus::default();
    let (_, source) = pending_interrupt(ControlIrq);
    let cpu = Arc::new(SoftwareCpu::new());
    let bound = cpu.bind_registers(&bus);
    let runtime = InterruptRuntime::new(cpu, source);
    let mut program = std::pin::pin!(runtime.run_main::<ControlIsr, _>(async |registration| {
        pdma_model_firmware::main(bound, SharedMemory::new(), registration).await
    }));
    let mut context = Context::from_waker(Waker::noop());
    // When: the executor polls the idle program repeatedly without a doorbell.
    for _ in 0..8 {
        assert!(program.as_mut().poll(&mut context).is_pending());
    }
    // Then: activation was read once and firmware remains suspended.
    assert_eq!(bus.0.load(Ordering::SeqCst), 1);
}
