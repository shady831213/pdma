module pdma_clock_domain #(
    parameter string NAME="reg",
    parameter string PATH="uvm_test_top.env.clocks.driver"
) (
    output wire clock,
    output wire reset_n,
    output wire rst_process
);
    import uvm_pkg::*;
    jvs_clk_group_if #(.name(NAME),.path(PATH)) group_if();
    bit reset_seen=0;
    assign clock=group_if.gen_clk_ifs[0].clock;
    assign rst_process=group_if.gen_clk_ifs[0].rst_process;
    // The library initializes reset high. Keep the DUT reset until the
    // requested power-up reset has actually asserted in this clock group.
    assign reset_n=reset_seen ? group_if.gen_clk_ifs[0].reset_n : 1'b0;
    always @(negedge group_if.gen_clk_ifs[0].reset_n)
        if(group_if.gen_clk_ifs[0].reset_n===1'b0) reset_seen=1;
    initial begin
        uvm_config_db#(virtual jvs_gen_clk_if)::set(null,"uvm_test_top.env.clocks",NAME,group_if.gen_clk_ifs[0]);
    end
endmodule
