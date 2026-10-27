use super::super::AccessError;
use super::super::HostIrq;
use super::*;
use crate::harness::runtime::platform::WINDOW_STATUS_BASE;
use hardware_interface_irq::{InterruptHandler, pending_interrupt};
use hardware_interface_register::RegisterAccess;
use hardware_interface_runtime::{InterruptRuntime, SoftwareCpu};
use pdma_model_software::control::ControlIrq;
use std::{future::pending, pin::pin, task::Poll};

struct FailedManagement;

struct TestHandler;

impl InterruptHandler<HostIrq> for TestHandler {
    type Error = Infallible;

    async fn handle(&mut self, _: &HostIrq) -> Result<(), Infallible> {
        Ok(())
    }
}

#[tokio::test]
async fn assembled_host_and_firmware_have_distinct_cpu_ownership() {
    // Given: the two actual CPU environments created by the managed assembler.
    let memory = hardware_interface_memory::SharedMemoryAccess::from(Arc::new(
        hardware_interface_memory::SharedMemory::new(),
    ));
    let (system, host) = Managed::new(memory).unwrap();
    let (owner, _) = host
        .runtime
        .run_main::<TestHandler, _>(async |_| {
            hardware_interface_runtime::CpuLocal::capture_active(0_u32).unwrap()
        })
        .await
        .unwrap();
    // When: firmware tries to use a Host-owned capability.
    let result = system
        .firmware_cpu
        .run_main(async {
            owner.with(|value| {
                *value += 1;
            })
        })
        .await;
    // Then: sharing the async executor did not merge the software CPU identities.
    assert_eq!(
        result,
        Err(hardware_interface_runtime::CpuContextError::Mismatch)
    );
}

impl RegisterAccess for FailedManagement {
    type Error = AccessError;
    async fn read(&self, _: u32, _: &mut [u8]) -> Result<(), AccessError> {
        Err(AccessError::Unmapped)
    }
    async fn write(&self, _: u32, _: &[u8]) -> Result<(), AccessError> {
        Err(AccessError::Unmapped)
    }
}

#[tokio::test]
async fn actor_failure_wins_over_ready_host_completion() {
    // Given: both results are ready in the same poll.
    let actor = async { Err::<Infallible, RunError>(RunError::HardwareStopped) };
    // When: the supervisor polls both actors.
    let result = supervise(actor, async { 42 }).await;
    // Then: a ready Host result cannot hide the hardware failure.
    assert!(matches!(result, Err(RunError::HardwareStopped)));
}

#[tokio::test]
async fn initialization_error_drops_other_scoped_actors() {
    // Given: a real firmware program with a failing management transport.
    let hardware_owned = Arc::new(());
    let host_owned = Arc::new(());
    let hardware_guard = Arc::clone(&hardware_owned);
    let host_guard = Arc::clone(&host_owned);
    let hardware = async move {
        let result = pending::<Result<Infallible, RunError>>().await;
        drop(hardware_guard);
        result
    };
    let firmware = Box::pin(async {
        let (_, source) = pending_interrupt(ControlIrq);
        let cpu = Arc::new(SoftwareCpu::new());
        let bus = cpu.bind_registers(FailedManagement);
        InterruptRuntime::new(cpu, source)
            .run_main(async |registration| {
                pdma_model_firmware::main(
                    bus,
                    hardware_interface_memory::SharedMemory::new(),
                    registration,
                    crate::harness::runtime::platform::ADDRESSES,
                )
                .await
            })
            .await
            .map_err(RunError::FirmwareInterrupt)?
            .0
            .map_err(RunError::Firmware)
    });
    let system = async { supervise(hardware, firmware).await? };
    // When: initialization fails while the Host and hardware remain pending.
    let result = supervise(system, async move {
        pending::<()>().await;
        drop(host_guard);
    })
    .await;
    // Then: the original failure propagates and no owned actor remains retained.
    assert!(matches!(
        result,
        Err(RunError::Firmware(pdma_model_firmware::Error::Bus(
            hardware_interface_runtime::CpuAccessError::Access(AccessError::Unmapped)
        )))
    ));
    assert_eq!(Arc::strong_count(&hardware_owned), 1);
    assert_eq!(Arc::strong_count(&host_owned), 1);
}

#[tokio::test]
async fn cancellation_drops_running_hardware_and_firmware() {
    // Given: firmware initialized a real system and all actor futures remain scoped.
    let memory = hardware_interface_memory::SharedMemoryAccess::from(Arc::new(
        hardware_interface_memory::SharedMemory::new(),
    ));
    let (system, host) = Managed::new(memory).unwrap();
    let retained = Arc::clone(&system.shared);
    let host_bus = host.bus;
    let firmware_cpu = Arc::clone(&system.firmware_cpu);
    {
        let mut running = pin!(system.run());
        let state = std::future::poll_fn(|cx| Poll::Ready(running.as_mut().poll(cx))).await;
        assert!(state.is_pending());
        let mut ready = [0; 4];
        host_bus.read(WINDOW_STATUS_BASE, &mut ready).await.unwrap();
        assert_eq!(ready, 0u32.to_le_bytes());
        // When: simulation is cancelled by dropping its scoped future.
    }
    // Then: only the explicitly retained probe and Host bus own the hardware.
    assert_eq!(Arc::strong_count(&retained), 2);
    assert_eq!(Arc::strong_count(&firmware_cpu), 1);
}
