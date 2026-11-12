// Observe custody at real mailbox ports; no assumption about relative clock phase
// or the number of cycles required by either synchronizer. Cold reset covers both ends.
module common_cell_mailbox_checker #(parameter type T=logic)(
    input logic s_clk, s_rst_n, s_valid, s_ready,
    input T s_data,
    input logic d_clk, d_rst_n, d_valid, d_ready,
    input T d_data
);
    longint unsigned sent=0, received=0;
    T snapshot;
    always @(posedge s_clk or negedge s_rst_n) begin
        if(!s_rst_n) sent<=0;
        else if(s_valid && s_ready) begin
            assert(sent==received) else $fatal(1,"CDC_CUSTODY_OVERWRITE");
            snapshot<=s_data;
            sent<=sent+1;
        end
    end
    always @(posedge d_clk or negedge d_rst_n) begin
        if(!d_rst_n) received<=0;
        else if(d_valid && d_ready) begin
            assert(sent==received+1) else $fatal(1,"CDC_CUSTODY_UNOWNED");
            assert(d_data===snapshot) else $fatal(1,"CDC_CUSTODY_DATA");
            received<=received+1;
        end
    end
    assert property(@(posedge d_clk) disable iff(!d_rst_n)
        d_valid && !d_ready |=> d_valid && $stable(d_data))
        else $fatal(1,"CDC_CUSTODY_BACKPRESSURE");
endmodule

// A bounded FIFO preserves ordered custody across arbitrary clock phase/ratio.
// This testbench queue is an independent oracle, not the DUT's Gray pointers.
module common_cell_async_queue_checker #(parameter type T=logic, parameter int DEPTH=16)(
    input logic s_clk, s_rst_n, s_valid, s_ready,
    input T s_data,
    input logic d_clk, d_rst_n, d_valid, d_ready,
    input T d_data
);
    T expected[$];
    longint unsigned sent=0,received=0;
    always @(posedge s_clk or negedge s_rst_n) begin
        if(!s_rst_n) begin sent=0; expected.delete(); end
        else if(s_valid && s_ready) begin
            assert(sent-received<DEPTH) else $fatal(1,"CDC_QUEUE_OVERFLOW");
            expected.push_back(s_data); sent++;
        end
    end
    always @(posedge d_clk or negedge d_rst_n) begin
        T value;
        if(!d_rst_n) received=0;
        else if(d_valid && d_ready) begin
            assert(expected.size()!=0) else $fatal(1,"CDC_QUEUE_UNOWNED");
            value=expected.pop_front();
            assert(d_data===value) else $fatal(1,"CDC_QUEUE_DATA");
            received++;
        end
    end
    assert property(@(posedge d_clk) disable iff(!d_rst_n)
        d_valid && !d_ready |=> d_valid && $stable(d_data))
        else $fatal(1,"CDC_QUEUE_BACKPRESSURE");
endmodule
