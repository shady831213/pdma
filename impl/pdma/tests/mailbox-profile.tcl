return [dict create top PdmaMailboxProof clocks {
    s_clk {period 2 reset s_rst_n} d_clk {period 1 reset d_rst_n}
} reset_groups {cold {s_rst_n d_rst_n}} mailboxes {
    mailbox {source s_clk destination d_clk max_delay 0.5}
} ports {
    s_clk {
        input {s_valid 1 s_response_ready 1 s_request 106}
        output {s_ready 1 s_response_valid 1 s_response 66}
    }
    d_clk {
        input {d_ready 1 d_response_valid 1 d_response 66}
        output {d_valid 1 d_response_ready 1 d_request 106}
    }
}]
