use super::super::{HostInterrupts, HostVector};
use hardware_interface_runtime::InterruptRuntime;
use pdma_model_host::{ManagementConfig, ManagementRings};
use pdma_model_software::control::{ControlIsr, InterruptEvent};

pub(super) async fn run<T>(
    runtime: InterruptRuntime<HostInterrupts>,
    program: impl AsyncFnOnce(InterruptEvent) -> T,
) -> T {
    runtime
        .run_main::<ControlIsr, _>(async |registration| {
            let interrupt = InterruptEvent::new();
            registration
                .register(HostVector::Control, interrupt.handler())
                .unwrap();
            program(interrupt).await
        })
        .await
        .unwrap()
        .0
}

pub(super) fn config(rings: ManagementRings, interrupt: &InterruptEvent) -> ManagementConfig {
    ManagementConfig {
        addresses: crate::harness::runtime::platform::ADDRESSES,
        rings,
        interrupt: interrupt.clone(),
    }
}
