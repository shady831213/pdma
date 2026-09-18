//! Runtime-bound MMIO protects firmware without application-side CPU guards.
#[doc = "Test platform register placement."]
pub mod platform;
use platform::ADDRESSES;

use hardware_interface_runtime::{
    CpuAccessError, CpuContextError, InterruptRuntime, RegisterAccess, SharedMemory, SoftwareCpu,
    pending_interrupt,
};
use pdma_model_software::control::{ControlIrq, ControlIsr};
use std::{
    future::{Future, poll_fn},
    io,
    sync::{
        Arc,
        atomic::{AtomicU32, AtomicUsize, Ordering},
    },
    task::Poll,
};

type TestResult = Result<(), Box<dyn std::error::Error>>;

struct SuspendedBus {
    pause_at: u32,
    writes: AtomicUsize,
    report: AtomicU32,
}

impl SuspendedBus {
    const fn new(pause_at: u32) -> Self {
        Self {
            pause_at,
            writes: AtomicUsize::new(0),
            report: AtomicU32::new(0),
        }
    }
}

impl RegisterAccess for &SuspendedBus {
    type Error = io::Error;

    async fn read(&self, address: u32, bytes: &mut [u8]) -> Result<(), Self::Error> {
        bytes.fill(0);
        if address == 0 {
            bytes.copy_from_slice(&1_u32.to_le_bytes());
        }
        Ok(())
    }

    async fn write(&self, address: u32, bytes: &[u8]) -> Result<(), Self::Error> {
        if address == self.pause_at {
            tokio::task::yield_now().await;
        }
        self.writes.fetch_add(1, Ordering::SeqCst);
        if address == ADDRESSES.firmware {
            self.report.store(
                u32::from_le_bytes(bytes.try_into().map_err(io::Error::other)?),
                Ordering::SeqCst,
            );
        }
        Ok(())
    }
}

async fn rejects_migrated_access(pause_at: u32, committed: usize) -> TestResult {
    // Given: firmware suspended before a boot MMIO side effect on its assigned CPU.
    let bus = SuspendedBus::new(pause_at);
    let cpu = Arc::new(SoftwareCpu::new());
    let bound = cpu.bind_registers(&bus);
    let (_, source) = pending_interrupt(ControlIrq);
    let runtime = InterruptRuntime::new(Arc::clone(&cpu), source);
    let (result, _) = runtime
        .run_main::<ControlIsr, _>(async |registration| {
            let mut firmware = std::pin::pin!(pdma_model_firmware::main(
                bound,
                SharedMemory::new(),
                registration,
                ADDRESSES,
            ));
            assert!(
                poll_fn(|cx| Poll::Ready(firmware.as_mut().poll(cx)))
                    .await
                    .is_pending()
            );
            assert_eq!(bus.writes.load(Ordering::SeqCst), committed);
            // When: a different modeled CPU polls that same firmware future.
            SoftwareCpu::new().run_main(firmware).await
        })
        .await?;
    // Then: the access capability rejects it without committing another write or boot report.
    assert!(
        matches!(
            result,
            Err(pdma_model_firmware::Error::Bus(CpuAccessError::Cpu(
                CpuContextError::Mismatch
            )))
        ),
        "{result:?}"
    );
    assert_eq!(bus.writes.load(Ordering::SeqCst), committed);
    assert_eq!(bus.report.load(Ordering::SeqCst), 0);
    Ok(())
}

#[tokio::test]
async fn migrated_window_write_is_rejected_by_the_runtime_port() -> TestResult {
    rejects_migrated_access(ADDRESSES.windows, 0).await
}

#[tokio::test]
async fn migrated_ready_write_is_rejected_by_the_runtime_port() -> TestResult {
    rejects_migrated_access(ADDRESSES.firmware, 1).await
}

#[tokio::test]
async fn runtime_reestablishes_the_firmware_cpu_on_every_poll() -> TestResult {
    // Given: firmware whose Ready publication suspends once.
    let bus = SuspendedBus::new(ADDRESSES.firmware);
    let cpu = Arc::new(SoftwareCpu::new());
    let bound = cpu.bind_registers(&bus);
    let (_, source) = pending_interrupt(ControlIrq);
    let runtime = InterruptRuntime::new(cpu, source);
    let mut running = std::pin::pin!(runtime.run_main::<ControlIsr, _>(async |registration| {
        pdma_model_firmware::main(bound, SharedMemory::new(), registration, ADDRESSES).await
    }));
    assert!(
        poll_fn(|cx| Poll::Ready(running.as_mut().poll(cx)))
            .await
            .is_pending()
    );
    // When: an outer caller with another CPU context drives the complete runtime again.
    assert!(
        SoftwareCpu::new()
            .run_main(poll_fn(|cx| Poll::Ready(running.as_mut().poll(cx))))
            .await
            .is_pending()
    );
    // Then: firmware still executes on its assigned CPU and publishes Ready normally.
    assert_eq!(bus.writes.load(Ordering::SeqCst), 2);
    assert_eq!(bus.report.load(Ordering::SeqCst), 1);
    Ok(())
}
