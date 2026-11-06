class pdma_scenario_coverage extends uvm_subscriber#(pdma_observation);
    `uvm_component_utils(pdma_scenario_coverage)
    int observations, completions;
    longint unsigned bytes;
    realtime latency_sum, latency_min=1s, latency_max;
    covergroup scenarios with function sample(pdma_observation item);
        option.per_instance=1;
        outcome: coverpoint item.outcome;
        length: coverpoint item.length {bins short_length={[0:16]}; bins medium_length={[17:64]}; bins long_length={[65:257]};}
        source_alignment: coverpoint item.source_lane {bins aligned={0}; bins unaligned={[1:7]};}
        dest_alignment: coverpoint item.dest_lane {bins aligned={0}; bins unaligned={[1:7]};}
        page_boundary: coverpoint item.cross_page;
        fence_width: coverpoint item.fence_wide;
    endgroup
    function new(string name,uvm_component parent); super.new(name,parent); scenarios=new; endfunction
    virtual function void write(pdma_observation t);
        scenarios.sample(t); observations++;
        if(t.outcome==TASK_SUCCESS) begin
            completions++; bytes+=t.length; latency_sum+=t.latency;
            if(t.latency<latency_min) latency_min=t.latency;
            if(t.latency>latency_max) latency_max=t.latency;
        end
    endfunction
    virtual function void report_phase(uvm_phase phase);
        if(observations==0) `uvm_error("VACUOUS_TEST","No scenario outcome observed")
        else if(completions==0)
            `uvm_info("OUTCOMES",$sformatf("observations=%0d successful_completions=0 coverage=%0.2f",observations,scenarios.get_inst_coverage()),UVM_NONE)
        else `uvm_info("PERFORMANCE",$sformatf("completions=%0d bytes=%0d latency_ns_min=%0.3f mean=%0.3f max=%0.3f coverage=%0.2f",
            completions,bytes,latency_min/1ns,latency_sum/completions/1ns,latency_max/1ns,scenarios.get_inst_coverage()),UVM_NONE)
    endfunction
endclass
