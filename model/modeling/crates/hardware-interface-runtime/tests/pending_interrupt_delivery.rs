//! Delivery from a pending source through the owning software CPU.

use hardware_interface_irq::{InterruptEnvelope, InterruptHandler};
use hardware_interface_runtime::{CpuContextError, CpuLocal, InterruptRuntime, SoftwareCpu};
use std::sync::Arc;
use tokio::sync::oneshot;

#[derive(Clone)]
struct Notice(u8);

impl InterruptEnvelope for Notice {
    type Vector = u8;

    fn vector(&self) -> u8 {
        self.0
    }
}

struct Handler {
    calls: CpuLocal<usize>,
    delivered: Option<oneshot::Sender<u8>>,
}

impl InterruptHandler<Notice> for Handler {
    type Error = CpuContextError;

    async fn handle(&mut self, notice: &Notice) -> Result<(), Self::Error> {
        self.calls.with(|calls| *calls += 1)?;
        if let Some(delivered) = self.delivered.take() {
            assert!(delivered.send(notice.vector()).is_ok());
        }
        Ok(())
    }
}

#[tokio::test]
async fn pending_source_delivers_through_the_owning_cpu_runtime() {
    // Given: a real pending source and CPU-local handler state.
    let cpu = Arc::new(SoftwareCpu::new());
    let calls = cpu.local(0);
    let (emitter, source) = hardware_interface_irq::pending_interrupt(Notice(7));
    let runtime = InterruptRuntime::new(Arc::clone(&cpu), source);
    // When: software registers its handler and hardware raises a notification.
    let (vector, summary) = runtime
        .run_main(async |interrupts| {
            let (delivered, received) = oneshot::channel();
            interrupts
                .register(
                    7,
                    Handler {
                        calls: calls.clone(),
                        delivered: Some(delivered),
                    },
                )
                .expect("register on the owning CPU");
            emitter.raise();
            received.await.expect("handler delivery")
        })
        .await
        .expect("runtime completion");
    // Then: the configured vector ran once with access to its owning CPU's state.
    assert_eq!(vector, 7);
    assert_eq!(summary.deliveries(), 1);
    assert_eq!(
        cpu.run_main(async { calls.with(|calls| *calls) }).await,
        Ok(1)
    );
}
