// Binding the common-cell ports covers both reg/core and core/bus links without
// depending on the private implementation of the synchronizer or payload layout.
bind common_cell_DuplexMailbox common_cell_mailbox_checker #(.T(REQUEST_TYPE)) request_custody(
    .s_clk(s_clk),.s_rst_n(s_rst_n),.s_valid(s_valid),.s_ready(s_ready),.s_data(s_request),
    .d_clk(d_clk),.d_rst_n(d_rst_n),.d_valid(d_valid),.d_ready(d_ready),.d_data(d_request)
);
bind common_cell_DuplexMailbox common_cell_mailbox_checker #(.T(RESPONSE_TYPE)) response_custody(
    .s_clk(d_clk),.s_rst_n(d_rst_n),.s_valid(d_response_valid),.s_ready(d_response_ready),.s_data(d_response),
    .d_clk(s_clk),.d_rst_n(s_rst_n),.d_valid(s_response_valid),.d_ready(s_response_ready),.d_data(s_response)
);

bind common_cell_AsyncQueue common_cell_async_queue_checker #(.T(TYPE),.DEPTH(DEPTH)) custody(
    .s_clk(s_clk),.s_rst_n(s_rst_n),.s_valid(s_valid),.s_ready(s_ready),.s_data(s_data),
    .d_clk(d_clk),.d_rst_n(d_rst_n),.d_valid(d_valid),.d_ready(d_ready),.d_data(d_data)
);
