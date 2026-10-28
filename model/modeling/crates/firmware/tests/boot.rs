//! Firmware boot failures at the software CPU and management transport boundaries.
#[doc = "Test platform register placement."]
pub mod platform;
use platform::ADDRESSES;

use hw_model_io::register::RegisterAccess;
use hw_model_irq::interrupt_wire;
use hw_model_swenv::{EnvContextError, InterruptRegistrationError, InterruptRuntime, SoftwareEnv};
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
    let (_, source) = interrupt_wire(ControlIrq);
    let cpu = Arc::new(SoftwareEnv::new());
    let bus = cpu.bind_registers(FailedBus);
    let runtime = InterruptRuntime::new(cpu, source);
    // When: firmware attempts initialization.
    let result = runtime
        .run_main::<ControlIsr, _>(async |registration| {
            pdma_model_firmware::main(
                bus,
                hw_model_io::memory::SharedMemory::new(),
                registration,
                ADDRESSES,
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
    let (_, source) = interrupt_wire(ControlIrq);
    let cpu = Arc::new(SoftwareEnv::new());
    let bus = cpu.bind_registers(FailedBus);
    let runtime = InterruptRuntime::new(cpu, source);
    // When: the program starts.
    let result = runtime
        .run_main::<ControlIsr, _>(async |registration| {
            SoftwareEnv::new()
                .run_main(pdma_model_firmware::main(
                    bus,
                    hw_model_io::memory::SharedMemory::new(),
                    registration,
                    ADDRESSES,
                ))
                .await
        })
        .await;
    // Then: it cannot perform management accesses as unowned runtime code.
    assert!(matches!(
        result,
        Ok((
            Err(pdma_model_firmware::Error::Registration(
                InterruptRegistrationError::Env(EnvContextError::Mismatch)
            )),
            _
        ))
    ));
}
