class pdma_scenario_coverage extends uvm_subscriber#(pdma_observation);
    `uvm_component_utils(pdma_scenario_coverage)
    int observations, completions;
    longint unsigned bytes;
    realtime latency_sum, latency_min=1s, latency_max;
    covergroup scenarios with function sample(pdma_observation item);
        option.per_instance=1;
        outcome: coverpoint item.outcome {
            bins task_results[]={TASK_SUCCESS,TASK_FAULT,TASK_ERROR,TASK_FATAL,TASK_RESET,TASK_ABORTED};
            illegal_bins service_only={IRQ_NOTIFICATION};
        }
        opcode: coverpoint item.opcode {
            bins copy_cmd={COMMANDHEADER_OPCODE_COPY}; bins write_cmd={COMMANDHEADER_OPCODE_WRITE};
            bins fill_cmd={COMMANDHEADER_OPCODE_FILL}; bins unsupported={COMMANDHEADER_OPCODE_ATOMIC};
            bins wait_cmd={COMMANDHEADER_OPCODE_WAIT};
            bins fence_cmd={COMMANDHEADER_OPCODE_FENCE}; bins timestamp_cmd={COMMANDHEADER_OPCODE_TIMESTAMP};
        }
        length: coverpoint item.length {bins short_length={[0:16]}; bins medium_length={[17:64]}; bins long_length={[65:$]};}
        source_alignment: coverpoint item.source_lane {bins aligned={0}; bins unaligned={[1:pdma_configuration::MEMORY_BEAT_BYTES-1]};}
        dest_alignment: coverpoint item.dest_lane {bins aligned={0}; bins unaligned={[1:pdma_configuration::MEMORY_BEAT_BYTES-1]};}
        page_boundary: coverpoint item.cross_page;
        event_boundary: coverpoint item.event_cross_page iff(item.outcome==TASK_ERROR);
        fence_width: coverpoint item.fence_wide;
    endgroup
    // Sample the decoded, checked event. A channel can stop even when no
    // event could be written, in which case there is no event code to count.
    covergroup error_events with function sample(int unsigned owner,int unsigned code);
        option.per_instance=1;
        channel: coverpoint owner {bins owners[]={[0:`PDMA_CHANNEL_COUNT-1]};}
        reason: coverpoint code {
            bins supported_codes[]={EVENT_CODE_INVALIDLENGTH,EVENT_CODE_READERROR,EVENT_CODE_WRITEERROR,
                EVENT_CODE_LENGTHMISMATCH,EVENT_CODE_LISTREADERROR,EVENT_CODE_UNSUPPORTED,EVENT_CODE_INVALIDOPERATION};
        }
        checked_errors: cross channel,reason;
    endgroup
    covergroup completed_copies with function sample(pdma_observation item);
        option.per_instance=1;
        channel: coverpoint item.channel_id {bins owners[]={[0:`PDMA_CHANNEL_COUNT-1]};}
        source_sg: coverpoint item.source_sg;
        dest_sg: coverpoint item.dest_sg;
        via_irq: coverpoint item.via_irq;
        checked_paths: cross channel, source_sg, dest_sg, via_irq;
    endgroup
    function new(string name,uvm_component parent);
        super.new(name,parent); scenarios=new; error_events=new; completed_copies=new;
    endfunction
    virtual function void write(pdma_observation t);
        scenarios.sample(t); observations++;
        if(t.outcome==TASK_ERROR) error_events.sample(t.channel_id,t.event_code);
        if(t.outcome==TASK_SUCCESS) begin
            if(t.opcode==COMMANDHEADER_OPCODE_COPY) completed_copies.sample(t);
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
