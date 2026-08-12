use pdma_model_contract::abi::mmio::{
    IrqClaimNotificationResponse, IrqLineSnapshotResponse, IrqMessageNotificationResponse,
    IrqResponse, IrqResponseBank, IrqSourceResponse, IrqSourceStatus,
};

fn status() -> IrqSourceStatus {
    match IrqSourceStatus::try_from(0x3b) {
        Ok(status) => status,
        Err(error) => {
            assert_eq!(error.bits, 0);
            IrqSourceStatus::default()
        }
    }
}

#[test]
fn irq_response_bank_has_source_and_distinct_empty_shapes() {
    // Given: a source response with route and pending-message state plus two empty outcomes.
    let source = IrqSourceResponse {
        epoch: 0x0102_0304_0506_0708,
        source_id: 0x1112_1314_1516_1718,
        source_kind: 0x2122_2324,
        state: status(),
        route_kind: 0x3132_3334,
        vector: 0x4142_4344_4546_4748,
        pending_messages: 0x5152_5354_5556_5758,
    };
    let responses = [
        IrqResponse::ConfigureRoute(source),
        IrqResponse::Enable(source),
        IrqResponse::Disable(source),
        IrqResponse::Mask(source),
        IrqResponse::Unmask(source),
        IrqResponse::Acknowledge(source),
        IrqResponse::ObserveSource(source),
    ];
    let expected = [
        0x08, 0x07, 0x06, 0x05, 0x04, 0x03, 0x02, 0x01, 0x18, 0x17, 0x16, 0x15, 0x14, 0x13, 0x12,
        0x11, 0x24, 0x23, 0x22, 0x21, 0x3b, 0, 0, 0, 0x34, 0x33, 0x32, 0x31, 0, 0, 0, 0, 0x48,
        0x47, 0x46, 0x45, 0x44, 0x43, 0x42, 0x41, 0, 0, 0, 0, 0, 0, 0, 0, 0x58, 0x57, 0x56, 0x55,
        0x54, 0x53, 0x52, 0x51,
    ];

    // When: source commands and both empty observation commands populate their banks.
    // Then: source fields are literal LE, irrelevant notification/message fields are zero, and
    // the two EMPTY outcomes remain distinct typed variants with all response fields zero.
    for response in responses {
        assert_eq!(
            response
                .bank()
                .to_le_bytes()
                .expect("generated response view must encode"),
            expected
        );
    }
    assert_eq!(
        IrqResponse::SnapshotLineEmpty.bank(),
        IrqResponseBank::default()
    );
    assert_eq!(
        IrqResponse::ClaimNotificationEmpty.bank(),
        IrqResponseBank::default()
    );
    assert_ne!(
        IrqResponse::SnapshotLineEmpty,
        IrqResponse::ClaimNotificationEmpty
    );
    println!(
        "typed IRQ response bank: 7 source shapes, SnapshotLine EMPTY, ClaimNotification EMPTY"
    );
}

#[test]
fn irq_response_bank_has_literal_notification_bytes_for_both_claim_kinds() {
    // Given: typed line and message observations with distinct notification fields.
    let line = IrqResponse::SnapshotLine(IrqLineSnapshotResponse {
        epoch: 0x0102_0304_0506_0708,
        source_id: 0x1112_1314_1516_1718,
        source_kind: 0x2122_2324,
        state: status(),
        route_kind: 0x3132_3334,
        vector: 0x4142_4344_4546_4748,
    });
    let message = IrqResponse::ClaimNotification(IrqClaimNotificationResponse::Message(
        IrqMessageNotificationResponse {
            epoch: 0x6162_6364_6566_6768,
            source_id: 0x7172_7374_7576_7778,
            source_kind: 0x8182_8384,
            state: status(),
            route_kind: 2,
            vector: 0x9192_9394_9596_9798,
            message_id: 0xa1a2_a3a4_a5a6_a7a8,
            pending_messages: 0xb1b2_b3b4_b5b6_b7b8,
        },
    ));

    // When: a line snapshot and a message notification are encoded.
    // Then: each command-specific response uses its literal notification shape and zero fill.
    assert_eq!(
        line.bank()
            .to_le_bytes()
            .expect("generated response view must encode"),
        [
            0x08, 0x07, 0x06, 0x05, 0x04, 0x03, 0x02, 0x01, 0x18, 0x17, 0x16, 0x15, 0x14, 0x13,
            0x12, 0x11, 0x24, 0x23, 0x22, 0x21, 0x3b, 0, 0, 0, 0x34, 0x33, 0x32, 0x31, 1, 0, 0, 0,
            0x48, 0x47, 0x46, 0x45, 0x44, 0x43, 0x42, 0x41, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0,
        ]
    );
    assert_eq!(
        message
            .bank()
            .to_le_bytes()
            .expect("generated response view must encode"),
        [
            0x68, 0x67, 0x66, 0x65, 0x64, 0x63, 0x62, 0x61, 0x78, 0x77, 0x76, 0x75, 0x74, 0x73,
            0x72, 0x71, 0x84, 0x83, 0x82, 0x81, 0x3b, 0, 0, 0, 2, 0, 0, 0, 2, 0, 0, 0, 0x98, 0x97,
            0x96, 0x95, 0x94, 0x93, 0x92, 0x91, 0xa8, 0xa7, 0xa6, 0xa5, 0xa4, 0xa3, 0xa2, 0xa1,
            0xb8, 0xb7, 0xb6, 0xb5, 0xb4, 0xb3, 0xb2, 0xb1,
        ]
    );
    println!("typed IRQ notification response: line and message literal LE shapes");
}

#[test]
fn closed_irq_response_table_covers_all_commands() {
    // Given: the closed successful IRQ response enum.
    let source = IrqSourceResponse::default();
    let codes = [
        IrqResponse::ConfigureRoute(source).command().code(),
        IrqResponse::Enable(source).command().code(),
        IrqResponse::Disable(source).command().code(),
        IrqResponse::Mask(source).command().code(),
        IrqResponse::Unmask(source).command().code(),
        IrqResponse::Acknowledge(source).command().code(),
        IrqResponse::ObserveSource(source).command().code(),
        IrqResponse::SnapshotLineEmpty.command().code(),
        IrqResponse::ClaimNotificationEmpty.command().code(),
    ];

    // When: each response shape selects its owning command.
    // Then: the complete literal IRQ command registry is covered.
    assert_eq!(codes, [1, 2, 3, 4, 5, 6, 7, 8, 9]);
    println!("closed typed response table coverage: IRQ=9");
}
