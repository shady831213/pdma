// Qualification of the passive CDC assertions against the real mailbox.
module CdcCustodyTest;
    timeunit 1ns; timeprecision 1ps;
    bit s_clk=0,d_clk=0,s_rst_n=0,d_rst_n=0;
    always #1 s_clk=~s_clk;
    initial begin #0.37; forever #1.5 d_clk=~d_clk; end
    bit s_valid=0,s_response_ready=0,d_ready=0,d_response_valid=0;
    logic s_ready,s_response_valid,d_valid,d_response_ready;
    logic [31:0] s_request=0,d_response=0,d_request,s_response;
    string mutation;
    common_cell_DuplexMailbox #(.REQUEST_TYPE(logic[31:0]),.RESPONSE_TYPE(logic[31:0])) dut(.*);
    task send_request(input logic [31:0] value);
        @(negedge s_clk);s_request=value;s_valid=1;
        do @(posedge s_clk); while(!s_ready);
        @(negedge s_clk);s_valid=0;s_request=0;
    endtask
    initial begin
        void'($value$plusargs("mutation=%s",mutation));
        repeat(4) @(negedge s_clk);
        s_rst_n=1;
        // Accept while the destination is still reset. Source owns the snapshot.
        send_request(32'h12345678);
        if(mutation=="overwrite") begin
            force dut.s_ready=1;
            send_request(32'h87654321);
        end
        repeat(4) @(negedge d_clk);
        d_rst_n=1;
        wait(d_valid);
        if(mutation=="data") begin
            // Corrupt before the first destination sample, isolating the data
            // check from the separate backpressure-stability property.
            force dut.d_request=0;
            d_ready=1;
            repeat(3) @(negedge d_clk);
        end
        repeat(3) @(negedge d_clk);
        if(mutation=="backpressure") begin
            force dut.d_request=0;
            repeat(3) @(negedge d_clk);
        end
        d_ready=1;
        @(negedge d_clk);d_ready=0;
        if(mutation=="duplicate") begin
            force dut.d_valid=1;
            d_ready=1;
            repeat(3) @(negedge d_clk);
        end
        repeat(8) @(negedge s_clk);
        send_request(32'hfedcba98);
        wait(d_valid);
        @(negedge d_clk);d_ready=1;
        @(negedge d_clk);d_ready=0;
        repeat(8) @(negedge s_clk);
        if(mutation!="") $fatal(1,"CDC_NEGATIVE_NOT_DETECTED");
        $display("PDMA_CDC_CUSTODY_PASS");
        $finish;
    end
    initial begin #2000; $fatal(1,"CDC_CUSTODY_TIMEOUT"); end
endmodule
