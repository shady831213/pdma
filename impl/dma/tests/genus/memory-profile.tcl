return [dict create top MemoryBoundaryTest clocks {clk {period 1 reset rst_n}} memories {
    first {instance first_ram provider tech_cell_generic_Ram1R1W kind Ram1R1W depth 8 width 13 clock clk}
    second {instance second_ram provider tech_cell_generic_Ram1R1W kind Ram1R1W depth 8 width 13 clock clk}
} ports {
    clk {
        input {read_enable 1 write_enable 1 read_address 3 write_address 3 first_data 13 second_data 13 write_mask 13}
        output {first_result 13 second_result 13}
    }
}]
