use super::*;
use hardware_interface_pipeline::AsyncInteractionBackend;
use pdma_model_contract::abi::registers::{IrqSourceRegisters, irq_offset, locate};
use pdma_model_contract::semantic::{CompletionRingId, IrqSource};

#[tokio::test(flavor = "current_thread")]
async fn intermediate_command_step_publishes_new_irq_readiness() {
    // Given: command work and a queue-owned source changed since the last aggregation.
    let fixture = accepted_copy();
    fixture
        .shared
        .backend
        .interact(|state| {
            for (register, value) in [
                (IrqSourceRegisters::ENABLED, 1),
                (IrqSourceRegisters::MASKED, 0),
            ] {
                state
                    .core
                    .write_mmio_register(
                        locate(irq_offset(0, 0, register.offset()).expect("offset"))
                            .expect("register"),
                        value,
                    )
                    .expect("source configuration");
            }
            state
                .core
                .queues
                .irq_source_mut(IrqSource::Completion(CompletionRingId::new(0)))
                .expect("queue source")
                .latch();
            assert!(!state.core.irq_notification_ready());
        })
        .await;
    let changes = fixture.shared.backend.subscribe_for_test();
    let mut operation = Operation::new(());

    // When: one real backend interaction fetches a command and evaluates IRQ output.
    let outcome = AsyncInteractionBackend::interact(&fixture.shared.backend, |state| {
        operation.step(&mut DeviceService::new(), state)
    })
    .await;

    // Then: IRQ waiters are notified before command completion, with no extra command step.
    assert!(matches!(outcome, Ok(StepOutcome::Progressed)));
    assert!(changes.has_changed().expect("watch remains open"));
    assert_eq!(observation(&fixture).await, (1, vec![0; 4], 0, 0));
    assert!(
        fixture
            .shared
            .backend
            .interact(|state| state.core.irq_notification_ready())
            .await
    );
}
