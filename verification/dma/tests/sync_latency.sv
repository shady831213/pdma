// Simulation-only replacement for the pinned basic synchronizer. Each instance
// independently defers a changed sample by zero or one destination clock. A Gray
// vector is sampled coherently: delaying unrelated bits arbitrarily would invent
// invalid encodings, not model the timing contract. This is a latency stress test,
// not an analog metastability or MTBF proof. Never synthesize this module.
module __std_synchronizer_basic #(
    parameter int unsigned WIDTH=8,
    parameter bit [WIDTH-1:0] INITIAL_VALUE='0,
    parameter int unsigned STAGES=2
)(
    input logic i_clk, i_rst,
    input logic [WIDTH-1:0] i_d,
    output logic [WIDTH-1:0] o_d
);
    logic [WIDTH-1:0] rg[STAGES];
    bit deferred;
    int unsigned random_state, initial_seed, signature;
    int unsigned injected, opportunities;
    function automatic int unsigned path_seed();
        string path=$sformatf("%m");
        int unsigned run_seed=1, value;
        void'($value$plusargs("ntb_random_seed=%d",run_seed));
        value=32'h811c9dc5 ^ run_seed;
        for(int k=0;k<path.len();k++) value=(value ^ int'(path[k]))*32'd16777619;
        return value==0 ? 1 : value;
    endfunction
    function automatic int unsigned advance_rng(input int unsigned value);
        value^=value<<13; value^=value>>17; value^=value<<5;
        return value;
    endfunction
    assign o_d=rg[STAGES-1];
    always @(posedge i_clk or negedge i_rst) begin
        if(!i_rst) begin
            foreach(rg[i]) rg[i]<=INITIAL_VALUE;
            deferred<=0; random_state<=path_seed(); initial_seed<=path_seed();
            injected<=0; opportunities<=0; signature<=0;
        end else begin
            for(int i=1;i<STAGES;i++) rg[i]<=rg[i-1];
            if(i_d!=rg[0] && !deferred) begin
                random_state<=advance_rng(random_state);
                opportunities<=opportunities+1;
                signature<=(signature<<1)^{31'b0,random_state[0]};
                if(random_state[0]) begin
                    deferred<=1; injected<=injected+1;
                end else rg[0]<=i_d;
            end else begin rg[0]<=i_d; deferred<=0; end
        end
    end
    final $display("CDC_LATENCY %m width=%0d seed=%08x opportunities=%0d injected=%0d signature=%08x",
        WIDTH,initial_seed,opportunities,injected,signature);
endmodule
