//! Management work is awakened by the software ISR, not by MMIO transport.

use hardware_interface_runtime::{
    InterruptDispatchError, InterruptRuntime, SoftwareCpu, pending_interrupt,
};
use pdma_model_software::control::{ControlIrq, ControlIsr, InterruptEvent};
use std::{
    future::{Future, pending, poll_fn},
    pin::pin,
    sync::Arc,
    task::Poll,
};

#[tokio::test]
async fn coalesced_interrupts_wake_the_registered_software_task() {
    // Given: one management interrupt source and a software program registering its ISR.
    let (emitter, source) = pending_interrupt(ControlIrq);
    let runtime = InterruptRuntime::new(Arc::new(SoftwareCpu::new()), source);
    let ((), summary) = runtime
        .run_main::<ControlIsr, _>(async |registration| {
            let event = InterruptEvent::new();
            registration.register((), event.handler()).unwrap();
            {
                let mut wait = pin!(event.wait());
                poll_fn(|cx| {
                    assert!(wait.as_mut().poll(cx).is_pending());
                    Poll::Ready(())
                })
                .await;
            }
            // When: hardware raises twice before software dispatch can claim the interrupt.
            emitter.raise();
            emitter.raise();
            event.wait().await;
            // Then: the ISR has woken the task, and no second wake is fabricated.
            let mut wait = pin!(event.wait());
            poll_fn(|cx| {
                assert!(wait.as_mut().poll(cx).is_pending());
                Poll::Ready(())
            })
            .await;
        })
        .await
        .unwrap();
    assert_eq!(summary.deliveries(), 1);
}

#[tokio::test]
async fn missing_handler_is_a_dispatch_error_not_a_direct_wakeup() {
    // Given: a raised management interrupt with no registered software ISR.
    let (emitter, source) = pending_interrupt(ControlIrq);
    emitter.raise();
    // When: a software program waits without installing its handler.
    let result = InterruptRuntime::new(Arc::new(SoftwareCpu::new()), source)
        .run_main::<ControlIsr, _>(async |_| pending::<()>().await)
        .await;
    // Then: the dispatcher reports the missing vector rather than bypassing software.
    assert!(matches!(
        result,
        Err(InterruptDispatchError::UnregisteredVector(()))
    ));
}
