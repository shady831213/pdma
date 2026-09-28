use super::super::{HostInterrupts, HostVector};
use hardware_interface_runtime::InterruptRuntime;
use pdma_model_host::{ManagementConfig, ManagementRings};
use pdma_model_software::control::{ControlIsr, InterruptEvent};

pub(super) async fn run<T>(
    runtime: InterruptRuntime<HostInterrupts>,
    program: impl AsyncFnOnce(Interrupts) -> T,
) -> T {
    runtime
        .run_main::<ControlIsr, _>(async |registration| {
            let interrupt = InterruptEvent::new();
            registration
                .register(HostVector::Control, interrupt.handler())
                .unwrap();
            let event = InterruptEvent::new();
            registration
                .register(HostVector::Event, event.handler())
                .unwrap();
            program(Interrupts {
                response: interrupt,
                event,
            })
            .await
        })
        .await
        .unwrap()
        .0
}

pub(super) fn config(rings: ManagementRings, interrupt: &Interrupts) -> ManagementConfig {
    ManagementConfig {
        addresses: crate::harness::runtime::platform::ADDRESSES,
        rings,
        interrupt: interrupt.response.clone(),
        event_interrupt: interrupt.event.clone(),
    }
}

pub(super) struct Interrupts {
    pub response: InterruptEvent,
    pub event: InterruptEvent,
}
