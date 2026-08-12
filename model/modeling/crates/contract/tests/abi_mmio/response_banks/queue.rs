use pdma_model_contract::abi::mmio::{QueueResponse, QueueResponseBank};

const QUEUE_WORDS: [u64; 9] = [
    0x0102_0304_0506_0708,
    0x1112_1314_1516_1718,
    0x2122_2324_2526_2728,
    0x3132_3334_3536_3738,
    0x4142_4344_4546_4748,
    0x5152_5354_5556_5758,
    0x6162_6364_6566_6768,
    0x7172_7374_7576_7778,
    0x8182_8384_8586_8788,
];

#[test]
fn queue_response_bank_publishes_all_nine_words_for_every_command() {
    // Given: a queue bank where every response word has a distinct literal byte pattern.
    let bank = QueueResponseBank {
        epoch: QUEUE_WORDS[0],
        queue_id: QUEUE_WORDS[1],
        sq_producer: QUEUE_WORDS[2],
        sq_captured: QUEUE_WORDS[3],
        sq_consumer: QUEUE_WORDS[4],
        cq_producer: QUEUE_WORDS[5],
        cq_consumer: QUEUE_WORDS[6],
        eq_producer: QUEUE_WORDS[7],
        eq_consumer: QUEUE_WORDS[8],
    };
    let responses = [
        QueueResponse::Activate(bank),
        QueueResponse::ReadStatus(bank),
        QueueResponse::PublishSqProducer(bank),
        QueueResponse::CaptureSqProducer(bank),
        QueueResponse::PublishSqConsumer(bank),
        QueueResponse::PublishCqProducer(bank),
        QueueResponse::PublishCqConsumer(bank),
        QueueResponse::PublishEqProducer(bank),
        QueueResponse::PublishEqConsumer(bank),
    ];
    let expected = [
        0x08, 0x07, 0x06, 0x05, 0x04, 0x03, 0x02, 0x01, 0x18, 0x17, 0x16, 0x15, 0x14, 0x13, 0x12,
        0x11, 0x28, 0x27, 0x26, 0x25, 0x24, 0x23, 0x22, 0x21, 0x38, 0x37, 0x36, 0x35, 0x34, 0x33,
        0x32, 0x31, 0x48, 0x47, 0x46, 0x45, 0x44, 0x43, 0x42, 0x41, 0x58, 0x57, 0x56, 0x55, 0x54,
        0x53, 0x52, 0x51, 0x68, 0x67, 0x66, 0x65, 0x64, 0x63, 0x62, 0x61, 0x78, 0x77, 0x76, 0x75,
        0x74, 0x73, 0x72, 0x71, 0x88, 0x87, 0x86, 0x85, 0x84, 0x83, 0x82, 0x81,
    ];

    // When: every queue command reports success.
    // Then: every success publishes the complete nine-u64 bank in literal LE order.
    for response in responses {
        assert_eq!(
            [
                response.bank().epoch,
                response.bank().queue_id,
                response.bank().sq_producer,
                response.bank().sq_captured,
                response.bank().sq_consumer,
                response.bank().cq_producer,
                response.bank().cq_consumer,
                response.bank().eq_producer,
                response.bank().eq_consumer,
            ],
            QUEUE_WORDS
        );
        assert_eq!(
            response
                .bank()
                .to_le_bytes()
                .expect("generated response view must encode"),
            expected
        );
    }
    println!("typed queue response bank: 9 command shapes publish all nine literal LE u64 words");
}

#[test]
fn closed_queue_response_table_covers_all_commands() {
    // Given: the closed successful queue response enum.
    let bank = QueueResponseBank::default();
    let codes = [
        QueueResponse::Activate(bank).command().code(),
        QueueResponse::ReadStatus(bank).command().code(),
        QueueResponse::PublishSqProducer(bank).command().code(),
        QueueResponse::CaptureSqProducer(bank).command().code(),
        QueueResponse::PublishSqConsumer(bank).command().code(),
        QueueResponse::PublishCqProducer(bank).command().code(),
        QueueResponse::PublishCqConsumer(bank).command().code(),
        QueueResponse::PublishEqProducer(bank).command().code(),
        QueueResponse::PublishEqConsumer(bank).command().code(),
    ];

    // When: each response shape selects its owning command.
    // Then: the complete literal queue command registry is covered.
    assert_eq!(codes, [1, 2, 3, 4, 5, 6, 7, 8, 9]);
    println!("closed typed response table coverage: queue=9");
}
