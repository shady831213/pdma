// Optional passive timing probes. Without +PDMA_INTERNAL_TIMING these initial
// blocks terminate: ordinary regressions pay no per-clock observation cost.
module pdma_pipeline_timing #(parameter int CHANNELS=1, SLOTS=8, localparam int SLOT_BITS=$clog2(SLOTS+2))(
    input logic core_clk, core_rst_n, bus_clk, bus_rst_n,
    input logic grant, input logic [2:0] chosen,
    input logic [CHANNELS-1:0] copy_start,
    input logic issued, input logic [2:0] issue_channel, input logic [SLOT_BITS-1:0] issue_slot,
    input logic issue_write,
    input logic retired, input logic [2:0] retire_channel, input logic [SLOT_BITS-1:0] retire_slot,
    input logic fetch_admitted, input logic [2:0] fetch_channel,
    input logic admitted, input logic [2:0] admit_channel, input logic [SLOT_BITS-1:0] admit_slot,
    input logic returned, input logic [2:0] return_channel, input logic [SLOT_BITS-1:0] return_slot
);
    timeunit 1ns; timeprecision 1ps;
    bit enabled=$test$plusargs("PDMA_INTERNAL_TIMING");
    // Each tag is individually ordered. Fetch tag zero may have several
    // ordered fragments on the wire, even though they fill a single descriptor.
    typedef struct { realtime issue_time; int category; } issue_stamp;
    issue_stamp awaiting_bus[CHANNELS][SLOTS+2][$], awaiting_response[CHANNELS][SLOTS+2][$];
    typedef struct { issue_stamp issued; realtime returned_at; } return_stamp;
    return_stamp awaiting_core[CHANNELS][SLOTS+2][$];
    bit grant_wait[CHANNELS], start_wait[CHANNELS];
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
        if(channel_>=CHANNELS || slot_>SLOTS+1) $fatal(1,"PIPELINE_TIMING_BAD_TAG");
    endfunction
    initial if(enabled) forever begin
        @(posedge core_clk);
        if(core_rst_n) begin
            // Receipt discharge precedes same-edge reuse, including R-to-W forwarding.
            if(retired) begin
                return_stamp done;
                valid_tag(retire_channel,retire_slot);
                if(!awaiting_core[retire_channel][retire_slot].size()) $fatal(1,"PIPELINE_TIMING_UNOWNED_RETURN");
                done=awaiting_core[retire_channel][retire_slot].pop_front();
                sample(1,done.issued.category,$realtime-done.returned_at);
                sample(2,done.issued.category,$realtime-done.issued.issue_time);
            end
            if(grant) begin
                valid_tag(chosen,0); grant_at[chosen]=$realtime; grant_wait[chosen]=1;
            end
            foreach(copy_start[c]) if(copy_start[c]) begin start_at[c]=$realtime; start_wait[c]=1; end
            if(issued) begin
                issue_stamp stamp;
                int category;
                valid_tag(issue_channel,issue_slot);
                category=(issue_slot inside {[1:SLOTS]} ? 2 : 0)+int'(issue_write);
                stamp.issue_time=$realtime; stamp.category=category;
                awaiting_bus[issue_channel][issue_slot].push_back(stamp);
                if(grant_wait[issue_channel]) begin
                    sample(3,category,$realtime-grant_at[issue_channel]); grant_wait[issue_channel]=0;
                end
                if((issue_slot inside {[1:SLOTS]}) && start_wait[issue_channel]) begin
                    sample(4,category,$realtime-start_at[issue_channel]); start_wait[issue_channel]=0;
                end
            end
        end
    end
    function automatic void admit(int channel_, int slot_);
        issue_stamp stamp;
        valid_tag(channel_,slot_);
        if(!awaiting_bus[channel_][slot_].size()) $fatal(1,"PIPELINE_TIMING_UNOWNED_ISSUE");
        stamp=awaiting_bus[channel_][slot_].pop_front();
        sample(0,stamp.category,$realtime-stamp.issue_time);
        awaiting_response[channel_][slot_].push_back(stamp);
    endfunction
    initial if(enabled) forever begin
        @(posedge bus_clk);
        if(bus_rst_n) begin
            if(fetch_admitted) admit(fetch_channel,0);
            if(admitted) admit(admit_channel,admit_slot);
            if(returned) begin
                return_stamp response;
                valid_tag(return_channel,return_slot);
                if(!awaiting_response[return_channel][return_slot].size()) $fatal(1,"PIPELINE_TIMING_DUPLICATE_RETURN");
                response.issued=awaiting_response[return_channel][return_slot].pop_front();
                response.returned_at=$realtime;
                awaiting_core[return_channel][return_slot].push_back(response);
            end
        end
    end
    final if(enabled) begin
        string names[5]='{"core_issue_to_endpoint","return_fifo_to_core","core_request_roundtrip",
                         "grant_to_first_issue","window_start_to_first_issue"};
        string classes[4]='{"control_read","control_write","payload_read","payload_write"};
        foreach(awaiting_bus[c,s]) if(awaiting_bus[c][s].size() || awaiting_response[c][s].size() || awaiting_core[c][s].size()) $error("PIPELINE_TIMING_UNFINISHED_REQUEST");
        foreach(samples[m,k]) if(samples[m][k]!=0)
            $display("PDMA_INTERNAL_TIMING metric=%s kind=%s samples=%0d min_ns=%0.3f mean_ns=%0.3f max_ns=%0.3f",
                names[m],classes[k],samples[m][k],minimum_[m][k],total_[m][k]/samples[m][k],maximum_[m][k]);
    end
endmodule
