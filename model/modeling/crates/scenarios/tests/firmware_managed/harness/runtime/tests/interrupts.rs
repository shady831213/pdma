use super::super::{HostIrq, HostVector, Managed, ManagementBus};
use super::control::memory::{kick, poll_once};
use crate::harness::runtime::platform::CONTROL_CHANNEL_BASE;
use hardware_interface_runtime::{
    CpuContextError, CpuOwner, InterruptDispatchError, InterruptEnvelope, InterruptHandler,
    InterruptSource, RegisterAccess, SharedMemory, SharedMemoryAccess,
};
use pdma_model_contract::ControlRegisters;
use pdma_model_software::control::{ControlIrq, ControlIsr, InterruptEvent};
use std::{convert::Infallible, future::pending, pin::pin, sync::Arc};

fn memory() -> SharedMemoryAccess {
    SharedMemoryAccess::from(Arc::new(SharedMemory::new()))
}

async fn response<M: Send>(bus: &ManagementBus<M>) {
    bus.write(
        CONTROL_CHANNEL_BASE + u32::try_from(ControlRegisters::RESPONSE.offset()).unwrap(),
        &1_u32.to_le_bytes(),
    )
    .await
    .unwrap();
}

#[tokio::test]
async fn control_doorbell_without_registered_isr_reports_dispatch_failure() {
    // Given: a real response doorbell pending before Host software starts.
    let (system, host) = Managed::new(memory()).unwrap();
    response(&ManagementBus(Arc::clone(&system.shared))).await;
    // When: Host runs without registering its control handler.
    let result = host
        .runtime
        .run_main::<ControlIsr, _>(async |_| pending::<()>().await)
        .await;
    // Then: canonical dispatch reports the missing control vector.
    assert!(matches!(
        result,
        Err(InterruptDispatchError::UnregisteredVector(
            HostVector::Control
        ))
    ));
}

#[tokio::test]
async fn repeated_response_doorbells_before_wait_coalesce_into_one_isr() {
    // Given: three response doorbells before software or its ISR has run.
    let (system, host) = Managed::new(memory()).unwrap();
    let bus = ManagementBus(Arc::clone(&system.shared));
    for _ in 0..3 {
        response(&bus).await;
    }
    // When: Host registers the ISR and then starts waiting.
    let ((), summary) = host
        .runtime
        .run_main::<ControlIsr, _>(async |registration| {
            let event = InterruptEvent::new();
            registration
                .register(HostVector::Control, event.handler())
                .unwrap();
            event.wait().await;
            let mut next = pin!(event.wait());
            assert!(poll_once(next.as_mut()).await.is_pending());
        })
        .await
        .unwrap();
    // Then: the pending platform message survived and was delivered only once.
    assert_eq!(summary.deliveries(), 1);
}

#[tokio::test]
async fn simultaneous_dma_and_control_pending_survive_cancelled_wait_and_are_fair() {
    // Given: separate pending bits with both destinations raised.
    let (dma, dma_source) =
        hardware_interface_runtime::pending_interrupt(HostIrq::Dma(pdma_model_host::runtime::Irq));
    let (control, control_source) =
        hardware_interface_runtime::pending_interrupt(HostIrq::Control(ControlIrq));
    let mut source = super::super::HostInterrupts {
        dma: dma_source,
        control: control_source,
        next: HostVector::Dma,
    };
    {
        let mut waiting = pin!(source.wait_ready());
        assert!(poll_once(waiting.as_mut()).await.is_pending());
    }
    dma.raise();
    control.raise();
    // When: a non-consuming wait is cancelled and DMA raises again after its claim.
    assert_eq!(source.wait_ready().await.unwrap(), HostVector::Dma);
    let ready = source.wait_ready().await.unwrap();
    assert_eq!(
        source.claim(ready).await.unwrap().unwrap().vector(),
        HostVector::Dma
    );
    dma.raise();
    // Then: control wins the next claim while the renewed DMA message remains pending.
    let ready = source.wait_ready().await.unwrap();
    assert_eq!(
        source.claim(ready).await.unwrap().unwrap().vector(),
        HostVector::Control
    );
    let ready = source.wait_ready().await.unwrap();
    assert_eq!(
        source.claim(ready).await.unwrap().unwrap().vector(),
        HostVector::Dma
    );
    let mut idle = pin!(source.wait_ready());
    assert!(poll_once(idle.as_mut()).await.is_pending());
}

struct OwnershipHandler {
    firmware: CpuOwner,
    host: CpuOwner,
    event: ControlIsr,
}

#[tokio::test]
async fn simultaneous_host_vectors_dispatch_to_separate_registered_events() {
    // Given: both platform destinations are pending on one actual Host runtime.
    let (system, host) = Managed::new(memory()).unwrap();
    system.shared.interrupt.raise();
    response(&ManagementBus(Arc::clone(&system.shared))).await;
    // When: software registers independent handlers and waits for both.
    let ((), summary) = host
        .runtime
        .run_main::<ControlIsr, _>(async |registration| {
            let dma = InterruptEvent::new();
            let control = InterruptEvent::new();
            registration
                .register(HostVector::Dma, dma.handler())
                .unwrap();
            registration
                .register(HostVector::Control, control.handler())
                .unwrap();
            dma.wait().await;
            control.wait().await;
        })
        .await
        .unwrap();
    // Then: each separate handler ran once despite simultaneous readiness.
    assert_eq!(summary.deliveries(), 2);
}

impl InterruptHandler<ControlIrq> for OwnershipHandler {
    type Error = Infallible;

    async fn handle(&mut self, irq: &ControlIrq) -> Result<(), Infallible> {
        self.firmware.validate_active().unwrap();
        assert_eq!(self.host.validate_active(), Err(CpuContextError::Mismatch));
        self.event.handle(irq).await
    }
}

#[tokio::test]
async fn request_doorbell_isr_runs_on_firmware_cpu() {
    // Given: the actual Host and firmware runtimes from one managed assembly.
    let (system, host) = Managed::new(memory()).unwrap();
    let (owner, _) = host
        .runtime
        .run_main::<ControlIsr, _>(async |_| CpuOwner::capture_active().unwrap())
        .await
        .unwrap();
    // When: a request doorbell is dispatched on firmware's runtime.
    kick(&host.bus).await.unwrap();
    let ((), summary) = system
        .firmware_runtime
        .run_main::<OwnershipHandler, _>(async |registration| {
            let event = InterruptEvent::new();
            registration
                .register(
                    (),
                    OwnershipHandler {
                        firmware: CpuOwner::capture_active().unwrap(),
                        host: owner,
                        event: event.handler(),
                    },
                )
                .unwrap();
            event.wait().await;
        })
        .await
        .unwrap();
    // Then: its handler validated firmware ownership and rejected Host ownership.
    assert_eq!(summary.deliveries(), 1);
}
