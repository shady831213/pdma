return [dict create top CdcIntegrationProbe clocks {
    s_clk {period 1 reset s_rst_n} d_clk {period 1 reset d_rst_n}
} reset_groups {cold {s_rst_n d_rst_n}} mailboxes {
    ip0/mailbox {source s_clk destination d_clk max_delay 0.5}
    ip01/mailbox {source s_clk destination d_clk max_delay 0.7}
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
