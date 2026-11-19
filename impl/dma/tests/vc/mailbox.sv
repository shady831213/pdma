// Proof boundary: the register mailbox and actual CSR payload types used by DmaCopy.
module PdmaMailboxProof (
    input logic s_clk, s_rst_n, d_clk, d_rst_n,
    input logic s_valid, s_response_ready, d_ready, d_response_valid,
    input common_cell___RegisterBus__32::Request s_request,
    input common_cell___RegisterBus__32::Response d_response,
    output logic s_ready, s_response_valid, d_valid, d_response_ready,
    output common_cell___RegisterBus__32::Request d_request,
    output common_cell___RegisterBus__32::Response s_response
);
    common_cell_DuplexMailbox #(
        .REQUEST_TYPE(common_cell___RegisterBus__32::Request),
        .RESPONSE_TYPE(common_cell___RegisterBus__32::Response)
    ) mailbox (.*);
endmodule
