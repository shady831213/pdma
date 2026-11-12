return [dict create top CdcIntegrationProbe clocks {
    s_clk {period 1 reset s_rst_n} d_clk {period 1 reset d_rst_n}
} reset_groups {cold {s_rst_n d_rst_n}} crossings {
    ip0/mailbox {provider common_cell cell DuplexMailbox source s_clk destination d_clk max_delay 0.5}
    ip01/requests {provider common_cell cell AsyncQueue source s_clk destination d_clk max_delay 0.7 depth 8}
    ip01/responses {provider common_cell cell AsyncQueue source d_clk destination s_clk max_delay 0.7 depth 8}
} ports {
    s_clk {
        input {s_valid 2 s_response_ready 2 s_request 16}
        output {s_ready 2 s_response_valid 2 s_response 16}
    }
    d_clk {
        input {d_ready 2 d_response_valid 2 d_response 16}
        output {d_valid 2 d_response_ready 2 d_request 16}
    }
}]
