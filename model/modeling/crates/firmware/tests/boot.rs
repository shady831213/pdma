//! Firmware boot failures at the software CPU and management transport boundaries.

use hardware_interface_runtime::{
    CpuContextError, InterruptRegistrationError, InterruptRuntime, RegisterAccess, SoftwareCpu,
    pending_interrupt,
};
use pdma_model_software::control::{ControlIrq, ControlIsr};
use std::{io, sync::Arc};

#[derive(Debug)]
struct FailedBus;

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
    let (_, source) = pending_interrupt(ControlIrq);
    let cpu = Arc::new(SoftwareCpu::new());
    let bus = cpu.bind_registers(FailedBus);
    let runtime = InterruptRuntime::new(cpu, source);
    // When: firmware attempts initialization.
    let result = runtime
        .run_main::<ControlIsr, _>(async |registration| {
            pdma_model_firmware::main(
                bus,
                hardware_interface_runtime::SharedMemory::new(),
                registration,
            )
            .await
        })
        .await;
    // Then: transport failure reaches the supervisor instead of reporting readiness.
    assert!(matches!(
        result,
        Ok((Err(pdma_model_firmware::Error::Bus(_)), _))
    ));
}

#[tokio::test]
async fn boot_requires_its_interrupt_registration_cpu() {
    // Given: firmware receives registration from a different software CPU.
    let (_, source) = pending_interrupt(ControlIrq);
    let cpu = Arc::new(SoftwareCpu::new());
    let bus = cpu.bind_registers(FailedBus);
    let runtime = InterruptRuntime::new(cpu, source);
    // When: the program starts.
    let result = runtime
        .run_main::<ControlIsr, _>(async |registration| {
            SoftwareCpu::new()
                .run_main(pdma_model_firmware::main(
                    bus,
                    hardware_interface_runtime::SharedMemory::new(),
                    registration,
                ))
                .await
        })
        .await;
    // Then: it cannot perform management accesses as unowned runtime code.
    assert!(matches!(
        result,
        Ok((
            Err(pdma_model_firmware::Error::Registration(
                InterruptRegistrationError::Cpu(CpuContextError::Mismatch)
            )),
            _
        ))
    ));
}
