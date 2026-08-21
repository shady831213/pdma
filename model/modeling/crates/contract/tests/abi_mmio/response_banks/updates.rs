use pdma_model_contract::abi::mmio::{
    GlobalResponse, GlobalResponseUpdate, IrqResponse, IrqResponseUpdate, IrqSourceResponse,
    IrqSourceStatus, QueueResponseBank, QueueResponseUpdate, apply_global_response_update,
    apply_irq_response_update, apply_queue_response_update,
};

#[test]
fn typed_response_failures_preserve_prior_bytes() {
    // Given: nonzero banks previously published by each MMIO block.
    let global = GlobalResponse::Discover {
        abi_version: 1,
        features: 0x0102_0304_0506_0708,
    }
    .bank();
    let queue = QueueResponseBank {
        epoch: 1,
        queue_id: 2,
        sq_producer: 3,
        sq_captured: 4,
        sq_consumer: 5,
        cq_producer: 6,
        cq_consumer: 7,
        eq_producer: 8,
        eq_consumer: 9,
    };
    let irq = IrqResponse::ObserveSource(IrqSourceResponse {
        epoch: 1,
        source_id: 2,
        source_kind: 3,
        state: match IrqSourceStatus::try_from(0x3b) {
            Ok(status) => status,
            Err(error) => {
                assert_eq!(error.bits, 0);
                IrqSourceStatus::default()
            }
        },
        route_kind: 4,
        vector: 5,
        pending_messages: 6,
    })
    .bank();

    // When: each command reaches a semantic failure update.
    // Then: the raw response bytes remain exactly unchanged.
    assert_eq!(
        apply_global_response_update(global, GlobalResponseUpdate::Preserve)
            .to_bytes()
            .expect("generated response view must encode"),
        global
            .to_bytes()
            .expect("generated response view must encode")
    );
    assert_eq!(
        apply_queue_response_update(queue, QueueResponseUpdate::Preserve)
            .to_bytes()
            .expect("generated response view must encode"),
        queue
            .to_bytes()
            .expect("generated response view must encode")
    );
    assert_eq!(
        apply_irq_response_update(irq, IrqResponseUpdate::Preserve)
            .to_bytes()
            .expect("generated response view must encode"),
        irq.to_bytes().expect("generated response view must encode")
    );
    println!("typed response semantic failures preserve prior global, queue, and IRQ bank bytes");
}
