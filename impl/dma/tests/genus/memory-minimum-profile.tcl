# Independent native one-bit/one-row fixture, including one-bit vector ports.
return [dict create top MemoryMinimumTest clocks {clk {period 1 reset rst_n}} memories {
    first {instance fixture/first_ram provider tech_cell_generic_Ram1R1W kind Ram1R1W depth 1 width 1 clock clk}
    second {instance fixture/second_ram provider tech_cell_generic_Ram1R1W kind Ram1R1W depth 1 width 1 clock clk}
} ports {
    clk {
        input {read_enable 1 write_enable 1 read_address 1 write_address 1 first_data 1 second_data 1 write_mask 1}
        output {first_result 1 second_result 1}
    }
}]
