use pdma_model_contract::abi::mmio::{
    GlobalResetResponse, GlobalResponse, IrqClaimNotificationResponse,
    IrqMessageNotificationResponse, IrqResponse, IrqSourceStatus, QueueResponseBank,
};

fn source_status() -> IrqSourceStatus {
    match IrqSourceStatus::try_from(0x3b) {
        Ok(status) => status,
        Err(error) => {
            assert_eq!(error.bits, 0);
            IrqSourceStatus::default()
        }
    }
}

#[test]
fn reset_response_is_epoch_only_with_literal_zero_fill() {
    // Given: a post-reset epoch whose bytes expose its exact global response lane.
    let response = GlobalResponse::Reset(GlobalResetResponse {
        epoch: 0x0102_0304_0506_0708,
    });

    // When: RESET populates its closed response shape.
    // Then: only epoch is populated and every non-epoch global field is literally zero.
    assert_eq!(
        response
            .bank()
            .to_bytes()
            .expect("generated response view must encode"),
        [
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x08, 0x07, 0x06, 0x05, 0x04, 0x03,
            0x02, 0x01, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        ]
    );
    println!("RESET response: epoch-only literal zero-fill");
}

#[test]
fn queue_response_bank_has_named_fields_and_literal_architecture_order() {
    // Given: every named queue response field has a distinct word pattern.
    let bank = QueueResponseBank {
        epoch: 0x0102_0304_0506_0708.into(),
        queue_id: 0x1112_1314_1516_1718.into(),
        sq_producer: 0x2122_2324_2526_2728.into(),
        sq_captured: 0x3132_3334_3536_3738.into(),
        sq_consumer: 0x4142_4344_4546_4748.into(),
        cq_producer: 0x5152_5354_5556_5758.into(),
        cq_consumer: 0x6162_6364_6566_6768.into(),
        eq_producer: 0x7172_7374_7576_7778.into(),
        eq_consumer: 0x8182_8384_8586_8788.into(),
    };

    // When: the typed named-field bank is encoded.
    // Then: all nine words appear in the architecture's literal register order.
    assert_eq!(bank.epoch.raw(), 0x0102_0304_0506_0708);
    assert_eq!(bank.queue_id.raw(), 0x1112_1314_1516_1718);
    assert_eq!(
        bank.to_bytes()
            .expect("generated response view must encode"),
        [
            0x08, 0x07, 0x06, 0x05, 0x04, 0x03, 0x02, 0x01, 0x18, 0x17, 0x16, 0x15, 0x14, 0x13,
            0x12, 0x11, 0x28, 0x27, 0x26, 0x25, 0x24, 0x23, 0x22, 0x21, 0x38, 0x37, 0x36, 0x35,
            0x34, 0x33, 0x32, 0x31, 0x48, 0x47, 0x46, 0x45, 0x44, 0x43, 0x42, 0x41, 0x58, 0x57,
            0x56, 0x55, 0x54, 0x53, 0x52, 0x51, 0x68, 0x67, 0x66, 0x65, 0x64, 0x63, 0x62, 0x61,
            0x78, 0x77, 0x76, 0x75, 0x74, 0x73, 0x72, 0x71, 0x88, 0x87, 0x86, 0x85, 0x84, 0x83,
            0x82, 0x81,
        ]
    );
    println!("typed queue named-field layout: epoch/id/seven cursors literal LE");
}

#[test]
fn message_claim_encodes_post_claim_source_status_and_message_route() {
    // Given: a message claim whose source state and Message route are nonzero.
    let response = IrqResponse::ClaimNotification(IrqClaimNotificationResponse::Message(
        IrqMessageNotificationResponse {
            epoch: 0x0102_0304_0506_0708,
            source_id: 0x1112_1314_1516_1718,
            source_kind: 0x2122_2324,
            state: source_status(),
            route_kind: 2,
            vector: 0x3132_3334_3536_3738,
            message_id: 0x4142_4344_4546_4748,
            pending_messages: 0x5152_5354_5556_5758,
        },
    ));

    // When: CLAIM_NOTIFICATION encodes a Message success.
    // Then: post-claim state and Message route occupy their literal IRQ response fields.
    assert_eq!(
        response
            .bank()
            .to_bytes()
            .expect("generated response view must encode"),
        [
            0x08, 0x07, 0x06, 0x05, 0x04, 0x03, 0x02, 0x01, 0x18, 0x17, 0x16, 0x15, 0x14, 0x13,
            0x12, 0x11, 0x24, 0x23, 0x22, 0x21, 0x3b, 0, 0, 0, 2, 0, 0, 0, 2, 0, 0, 0, 0x38, 0x37,
            0x36, 0x35, 0x34, 0x33, 0x32, 0x31, 0x48, 0x47, 0x46, 0x45, 0x44, 0x43, 0x42, 0x41,
            0x58, 0x57, 0x56, 0x55, 0x54, 0x53, 0x52, 0x51,
        ]
    );
    println!("Message claim: post-source-status plus Message route literal LE");
}
