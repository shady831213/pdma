use pdma_model_contract::abi::mmio::{
    GlobalLimitsResponse, GlobalResetResponse, GlobalResponse, GlobalStatusResponse,
};

#[test]
fn global_response_bank_has_literal_le_bytes_for_all_commands() {
    // Given: four response shapes with values that expose every byte order and zero fill rule.
    let responses = [
        (
            GlobalResponse::Discover {
                abi_version: 0x1122_3344,
                features: 0x0102_0304_0506_0708,
            },
            [
                0x44, 0x33, 0x22, 0x11, 0, 0, 0, 0, 0x08, 0x07, 0x06, 0x05, 0x04, 0x03, 0x02, 0x01,
                0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            ],
        ),
        (
            GlobalResponse::ReadLimits(GlobalLimitsResponse {
                max_ring_depth: 0x1122_3344,
                max_indirect_destinations: 0x5566_7788,
                max_transfer_bytes: 0x99aa_bbcc,
                max_strided_rows: 0xddee_ff00,
                max_stride: 0x1234_5678,
            }),
            [
                0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x44, 0x33,
                0x22, 0x11, 0x88, 0x77, 0x66, 0x55, 0xcc, 0xbb, 0xaa, 0x99, 0, 0xff, 0xee, 0xdd,
                0x78, 0x56, 0x34, 0x12,
            ],
        ),
        (
            GlobalResponse::ReadStatus(GlobalStatusResponse {
                run_state: 0x1122_3344,
                epoch: 0x0102_0304_0506_0708,
            }),
            [
                0, 0, 0, 0, 0x44, 0x33, 0x22, 0x11, 0, 0, 0, 0, 0, 0, 0, 0, 0x08, 0x07, 0x06, 0x05,
                0x04, 0x03, 0x02, 0x01, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            ],
        ),
        (
            GlobalResponse::Reset(GlobalResetResponse {
                epoch: 0x1112_1314_1516_1718,
            }),
            [
                0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x18, 0x17, 0x16, 0x15, 0x14, 0x13,
                0x12, 0x11, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            ],
        ),
    ];

    // When: each closed global command response populates a response bank.
    // Then: every raw byte is literal LE and every irrelevant field is zero.
    for (response, expected) in responses {
        assert_eq!(
            response
                .bank()
                .to_le_bytes()
                .expect("generated response view must encode"),
            expected
        );
    }
    println!("typed global response bank: 4 command shapes with literal LE bytes and zero-fill");
}

#[test]
fn closed_global_response_table_covers_all_commands() {
    // Given: the closed successful global response enum.
    let codes = [
        GlobalResponse::Discover {
            abi_version: 1,
            features: 2,
        }
        .command()
        .code(),
        GlobalResponse::ReadLimits(GlobalLimitsResponse::default())
            .command()
            .code(),
        GlobalResponse::ReadStatus(GlobalStatusResponse::default())
            .command()
            .code(),
        GlobalResponse::Reset(GlobalResetResponse::default())
            .command()
            .code(),
    ];

    // When: each response shape selects its owning command.
    // Then: the complete literal global command registry is covered.
    assert_eq!(codes, [1, 2, 3, 4]);
    println!("closed typed response table coverage: global=4");
}
