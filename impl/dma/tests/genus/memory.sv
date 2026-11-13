// Two equal-shaped providers with distinct roles must remain distinguishable
// after linking and hierarchy optimization. Functional behavior is tested in Veryl.
module MemoryBoundaryTest #(
    parameter int DEPTH=8, WIDTH=13,
    parameter int ADDR_WIDTH=DEPTH>1?$clog2(DEPTH):1
)(
    input logic clk, rst_n, read_enable, write_enable,
    input logic [ADDR_WIDTH-1:0] read_address, write_address,
    input logic [WIDTH-1:0] first_data, second_data, write_mask,
    output logic [WIDTH-1:0] first_result, second_result
);
    common_cell_RamReadPort #(.ADDR_WIDTH(ADDR_WIDTH),.WIDTH(WIDTH)) first_read(), second_read();
    common_cell_RamWritePort #(.ADDR_WIDTH(ADDR_WIDTH),.WIDTH(WIDTH)) first_write(), second_write();
    assign first_read.enable=read_enable && rst_n;
    assign second_read.enable=read_enable && rst_n;
    assign first_read.address=read_address;
    assign second_read.address=read_address;
    assign first_write.enable=write_enable && rst_n;
    assign second_write.enable=write_enable && rst_n;
    assign first_write.address=write_address;
    assign second_write.address=write_address;
    assign first_write.data=first_data;
    assign second_write.data=second_data;
    assign first_write.mask=write_mask;
    assign second_write.mask=write_mask;
    assign first_result=first_read.data;
    assign second_result=second_read.data;
    tech_cell_generic_Ram1R1W #(.DEPTH(DEPTH),.WIDTH(WIDTH)) first_ram(
        .clk(clk),.read_port(first_read),.write_port(first_write));
    tech_cell_generic_Ram1R1W #(.DEPTH(DEPTH),.WIDTH(WIDTH)) second_ram(
        .clk(clk),.read_port(second_read),.write_port(second_write));
endmodule

module MemoryMinimumTest(
    input logic clk, rst_n, read_enable, write_enable,
    input logic [0:0] read_address, write_address, first_data, second_data, write_mask,
    output logic [0:0] first_result, second_result
);
    MemoryBoundaryTest #(.DEPTH(1),.WIDTH(1)) fixture(.*);
endmodule
