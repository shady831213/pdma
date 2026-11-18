// Optional passive timing probes. Without +PDMA_INTERNAL_TIMING these initial
// blocks terminate: ordinary regressions pay no per-clock observation cost.
module pdma_pipeline_timing #(parameter int CHANNELS=1, SLOTS=8, localparam int SLOT_BITS=$clog2(SLOTS+1))(
    input logic core_clk, core_rst_n, bus_clk, bus_rst_n,
    input logic grant, input logic [2:0] chosen,
    input logic [CHANNELS-1:0] copy_start,
    input logic issued, input logic [2:0] issue_channel, input logic [SLOT_BITS-1:0] issue_slot,
    input logic issue_write,
    input logic retired, input logic [2:0] retire_channel, input logic [SLOT_BITS-1:0] retire_slot,
    input logic admitted, input logic [2:0] admit_channel, input logic [SLOT_BITS-1:0] admit_slot,
    input logic returned, input logic [2:0] return_channel, input logic [SLOT_BITS-1:0] return_slot
);
    timeunit 1ns; timeprecision 1ps;
    bit enabled=$test$plusargs("PDMA_INTERNAL_TIMING");
    bit outstanding[CHANNELS][SLOTS+1], return_seen[CHANNELS][SLOTS+1];
    bit grant_wait[CHANNELS], start_wait[CHANNELS];
    int kind[CHANNELS][SLOTS+1];
    realtime issue_at[CHANNELS][SLOTS+1], return_at[CHANNELS][SLOTS+1];
    realtime grant_at[CHANNELS], start_at[CHANNELS];
    int unsigned samples[5][4];
    realtime minimum_[5][4], maximum_[5][4], total_[5][4];
    function automatic void sample(int metric, int category, realtime duration);
        if(duration<0) $fatal(1,"PIPELINE_TIMING_NEGATIVE_INTERVAL");
        if(samples[metric][category]==0 || duration<minimum_[metric][category]) minimum_[metric][category]=duration;
        if(samples[metric][category]==0 || duration>maximum_[metric][category]) maximum_[metric][category]=duration;
        samples[metric][category]++; total_[metric][category]+=duration;
    endfunction
    function automatic void valid_tag(int channel_, int slot_);
        if(channel_>=CHANNELS || slot_>SLOTS) $fatal(1,"PIPELINE_TIMING_BAD_TAG");
    endfunction
    initial if(enabled) forever begin
        @(posedge core_clk);
        if(core_rst_n) begin
            // Receipt discharge precedes same-edge reuse, including R-to-W forwarding.
            if(retired) begin
                valid_tag(retire_channel,retire_slot);
                if(!outstanding[retire_channel][retire_slot] || !return_seen[retire_channel][retire_slot])
                    $fatal(1,"PIPELINE_TIMING_UNOWNED_RETURN");
                sample(1,kind[retire_channel][retire_slot],$realtime-return_at[retire_channel][retire_slot]);
                sample(2,kind[retire_channel][retire_slot],$realtime-issue_at[retire_channel][retire_slot]);
                outstanding[retire_channel][retire_slot]=0;
            end
            if(grant) begin
                valid_tag(chosen,0); grant_at[chosen]=$realtime; grant_wait[chosen]=1;
            end
            foreach(copy_start[c]) if(copy_start[c]) begin start_at[c]=$realtime; start_wait[c]=1; end
            if(issued) begin
                int category;
                valid_tag(issue_channel,issue_slot);
                if(outstanding[issue_channel][issue_slot]) $fatal(1,"PIPELINE_TIMING_EARLY_REUSE");
                category=(issue_slot!=0 ? 2 : 0)+int'(issue_write);
                kind[issue_channel][issue_slot]=category;
                issue_at[issue_channel][issue_slot]=$realtime;
                outstanding[issue_channel][issue_slot]=1; return_seen[issue_channel][issue_slot]=0;
                if(grant_wait[issue_channel]) begin
                    sample(3,category,$realtime-grant_at[issue_channel]); grant_wait[issue_channel]=0;
                end
                if(issue_slot!=0 && start_wait[issue_channel]) begin
                    sample(4,category,$realtime-start_at[issue_channel]); start_wait[issue_channel]=0;
                end
            end
        end
    end
    initial if(enabled) forever begin
        @(posedge bus_clk);
        if(bus_rst_n) begin
            if(admitted) begin
                valid_tag(admit_channel,admit_slot);
                if(!outstanding[admit_channel][admit_slot]) $fatal(1,"PIPELINE_TIMING_UNOWNED_ISSUE");
                sample(0,kind[admit_channel][admit_slot],$realtime-issue_at[admit_channel][admit_slot]);
            end
            if(returned) begin
                valid_tag(return_channel,return_slot);
                if(!outstanding[return_channel][return_slot] || return_seen[return_channel][return_slot])
                    $fatal(1,"PIPELINE_TIMING_DUPLICATE_RETURN");
                return_at[return_channel][return_slot]=$realtime; return_seen[return_channel][return_slot]=1;
            end
        end
    end
    final if(enabled) begin
        string names[5]='{"core_issue_to_endpoint","return_fifo_to_core","core_request_roundtrip",
                         "grant_to_first_issue","window_start_to_first_issue"};
        string classes[4]='{"control_read","control_write","payload_read","payload_write"};
        foreach(outstanding[c,s]) if(outstanding[c][s]) $error("PIPELINE_TIMING_UNFINISHED_REQUEST");
        foreach(samples[m,k]) if(samples[m][k]!=0)
            $display("PDMA_INTERNAL_TIMING metric=%s kind=%s samples=%0d min_ns=%0.3f mean_ns=%0.3f max_ns=%0.3f",
                names[m],classes[k],samples[m][k],minimum_[m][k],total_[m][k]/samples[m][k],maximum_[m][k]);
    end
endmodule
