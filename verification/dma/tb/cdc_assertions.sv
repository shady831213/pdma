// Observe custody at real mailbox ports; no assumption about relative clock phase
// or the number of cycles required by either synchronizer. Cold reset covers both ends.
module pdma_mailbox_custody #(parameter type T=logic)(
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

// Binding the common-cell ports covers both reg/core and core/bus links without
// depending on the private implementation of the synchronizer or payload layout.
bind common_cell_DuplexMailbox pdma_mailbox_custody #(.T(REQUEST_TYPE)) request_custody(
    .s_clk(s_clk),.s_rst_n(s_rst_n),.s_valid(s_valid),.s_ready(s_ready),.s_data(s_request),
    .d_clk(d_clk),.d_rst_n(d_rst_n),.d_valid(d_valid),.d_ready(d_ready),.d_data(d_request)
);
bind common_cell_DuplexMailbox pdma_mailbox_custody #(.T(RESPONSE_TYPE)) response_custody(
    .s_clk(d_clk),.s_rst_n(d_rst_n),.s_valid(d_response_valid),.s_ready(d_response_ready),.s_data(d_response),
    .d_clk(s_clk),.d_rst_n(s_rst_n),.d_valid(s_response_valid),.d_ready(s_response_ready),.d_data(s_response)
);
