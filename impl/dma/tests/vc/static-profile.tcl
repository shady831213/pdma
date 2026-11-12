return [dict create top StaticProbe clocks {
    s_clk {period 10 reset s_rst_n}
    d_clk {period 14 reset d_rst_n}
} ports {
    s_clk {input {s_data 1}}
    d_clk {output {d_data 1}}
}]
