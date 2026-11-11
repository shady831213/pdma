// Proof boundary: the exact reusable mailbox and payload types used by DmaCopy.
module PdmaMailboxProof (
    input logic s_clk, s_rst_n, d_clk, d_rst_n,
    input logic s_valid, s_response_ready, d_ready, d_response_valid,
    input pdma_rtl_CopyMemory::Request s_request,
    input pdma_rtl_CopyMemory::Receipt d_response,
    output logic s_ready, s_response_valid, d_valid, d_response_ready,
    output pdma_rtl_CopyMemory::Request d_request,
    output pdma_rtl_CopyMemory::Receipt s_response
);
    common_cell_DuplexMailbox #(
        .REQUEST_TYPE(pdma_rtl_CopyMemory::Request),
        .RESPONSE_TYPE(pdma_rtl_CopyMemory::Receipt)
    ) mailbox (.*);
endmodule
