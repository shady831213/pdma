// Optional transaction-level measurements at the external memory pins. Compare
// accepted addresses with terminal responses; an early final R or B completes
// the oldest transfer of that ID even when older transfers of other IDs remain.
module pdma_axi_activity #(parameter int ID_BITS=1) (
    input logic clk, rst_n, ar, aw, r, rlast, b,
    input logic [ID_BITS-1:0] arid, awid, rid, bid
);
    typedef bit [ID_BITS-1:0] id_t;
    id_t read_ids[$], write_ids[$], previous_rid;
    bit enabled=$test$plusargs("PDMA_AXI_ACTIVITY"), unfinished_burst;
    int unsigned read_peak, write_peak, r_overtakes, b_overtakes, r_interleaves;

    function automatic int retire(ref id_t issued[$], input id_t id);
        foreach(issued[i]) if(issued[i]==id) begin
            issued.delete(i);
            return int'(i!=0);
        end
        $fatal(1,"AXI_ACTIVITY_UNOWNED_ID id=%0d",id);
        return 0;
    endfunction

    initial if(enabled) forever begin
        @(posedge clk);
        if(!rst_n) begin
            read_ids.delete(); write_ids.delete(); unfinished_burst=0;
        end else begin
            if(r) begin
                if(unfinished_burst && previous_rid!=rid) r_interleaves++;
                unfinished_burst=!rlast; previous_rid=rid;
                if(rlast) r_overtakes+=retire(read_ids,rid);
            end
            if(b) b_overtakes+=retire(write_ids,bid);
            if(ar) read_ids.push_back(arid);
            if(aw) write_ids.push_back(awid);
            if(read_ids.size()>read_peak) read_peak=read_ids.size();
            if(write_ids.size()>write_peak) write_peak=write_ids.size();
        end
    end
    final if(enabled)
        $display("PDMA_AXI_ACTIVITY peak_AR=%0d peak_AW=%0d RLAST_overtakes=%0d B_overtakes=%0d interleaved_R_switches=%0d",
            read_peak,write_peak,r_overtakes,b_overtakes,r_interleaves);
endmodule
