// Two IP instances share clocks but have different timing budgets. Changing
// ip0/mailbox must not change ip01/mailbox or create/replace chip clocks.
module CdcIntegrationBlock (
    input logic s_clk, s_rst_n, d_clk, d_rst_n,
    input logic s_valid, s_response_ready, d_ready, d_response_valid,
    input logic [7:0] s_request, d_response,
    output logic s_ready, s_response_valid, d_valid, d_response_ready,
    output logic [7:0] d_request, s_response
);
    // A real integration reset qualifier also exercises asynchronous assertion
    // paths into data/enable logic, in addition to the flops' reset pins.
    common_cell_DuplexMailbox #(.REQUEST_TYPE(logic[7:0]),.RESPONSE_TYPE(logic[7:0])) mailbox(
        .s_valid(s_valid && s_rst_n),.d_response_valid(d_response_valid && d_rst_n),.*
    );
endmodule
module QueueIntegrationBlock (
    input logic s_clk, s_rst_n, d_clk, d_rst_n,
    input logic s_valid, s_response_ready, d_ready, d_response_valid,
    input logic [7:0] s_request, d_response,
    output logic s_ready, s_response_valid, d_valid, d_response_ready,
    output logic [7:0] d_request, s_response
);
    common_cell_AsyncQueue #(.TYPE(logic[7:0]),.DEPTH(8)) requests(
        .s_clk,.s_rst_n,.s_valid,.s_ready,.s_data(s_request),
        .d_clk,.d_rst_n,.d_valid,.d_ready,.d_data(d_request)
    );
    common_cell_AsyncQueue #(.TYPE(logic[7:0]),.DEPTH(8)) responses(
        .s_clk(d_clk),.s_rst_n(d_rst_n),.s_valid(d_response_valid),
        .s_ready(d_response_ready),.s_data(d_response),
        .d_clk(s_clk),.d_rst_n(s_rst_n),.d_valid(s_response_valid),
        .d_ready(s_response_ready),.d_data(s_response)
    );
endmodule
module CdcIntegrationProbe (
    input logic s_clk, s_rst_n, d_clk, d_rst_n,
    input logic [1:0] s_valid, s_response_ready, d_ready, d_response_valid,
    input logic [15:0] s_request, d_response,
    output logic [1:0] s_ready, s_response_valid, d_valid, d_response_ready,
    output logic [15:0] d_request, s_response
);
    CdcIntegrationBlock ip0(
        .s_clk,.s_rst_n,.d_clk,.d_rst_n,
        .s_valid(s_valid[0]),.s_response_ready(s_response_ready[0]),
        .d_ready(d_ready[0]),.d_response_valid(d_response_valid[0]),
        .s_request(s_request[7:0]),.d_response(d_response[7:0]),
        .s_ready(s_ready[0]),.s_response_valid(s_response_valid[0]),
        .d_valid(d_valid[0]),.d_response_ready(d_response_ready[0]),
        .d_request(d_request[7:0]),.s_response(s_response[7:0])
    );
    QueueIntegrationBlock ip01(
        .s_clk,.s_rst_n,.d_clk,.d_rst_n,
        .s_valid(s_valid[1]),.s_response_ready(s_response_ready[1]),
        .d_ready(d_ready[1]),.d_response_valid(d_response_valid[1]),
        .s_request(s_request[15:8]),.d_response(d_response[15:8]),
        .s_ready(s_ready[1]),.s_response_valid(s_response_valid[1]),
        .d_valid(d_valid[1]),.d_response_ready(d_response_ready[1]),
        .d_request(d_request[15:8]),.s_response(s_response[15:8])
    );
endmodule
