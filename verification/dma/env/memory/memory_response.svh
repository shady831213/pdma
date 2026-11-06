class pdma_memory_response extends svt_axi_slave_base_sequence;
    `uvm_object_utils(pdma_memory_response)
    function new(string name="memory_response"); super.new(name); endfunction
    virtual task body();
        svt_configuration raw;
        svt_axi_slave_transaction response;
        pdma_configuration behavior;
        pdma_memory memory;
        bit inject;
        if(!uvm_config_db#(pdma_configuration)::get(p_sequencer,"","behavior",behavior))
            `uvm_fatal("MEMORY_CONFIG","Missing responder configuration")
        if(!uvm_config_db#(pdma_memory)::get(p_sequencer,"","memory",memory))
            `uvm_fatal("MEMORY_CONFIG","Missing platform memory permissions")
        p_sequencer.get_cfg(raw);
        if(!$cast(cfg,raw)) `uvm_fatal("MEMORY_CONFIG","Unexpected VIP configuration")
        instantiate_axi_slave_mem();
        sink_responses();
        forever begin
            p_sequencer.response_request_port.peek(response);
            memory.assert_access(response.addr,response.burst_length*(1<<response.burst_size),
                response.addr_user,response.xact_type==svt_axi_transaction::WRITE);
            inject=behavior.fault_enabled && response.addr==behavior.fault_address &&
                ((response.xact_type==svt_axi_transaction::WRITE)==behavior.fault_write);
            if(!response.randomize() with {
                bresp==(local::inject ? svt_axi_transaction::SLVERR : svt_axi_transaction::OKAY);
                foreach(rresp[i]) rresp[i]==(local::inject ? svt_axi_transaction::SLVERR : svt_axi_transaction::OKAY);
                addr_ready_delay inside {[0:local::behavior.delay_max]};
                bvalid_delay inside {[0:local::behavior.delay_max]};
                foreach(wready_delay[i]) wready_delay[i] inside {[0:local::behavior.delay_max]};
                foreach(rvalid_delay[i]) rvalid_delay[i] inside {[0:local::behavior.delay_max]};
                enable_interleave==0;
                resp_user==(local::inject && local::behavior.fault_page ? 1 : 0);
                foreach(data_user[i]) data_user[i]==(local::inject && local::behavior.fault_page ? 1 : 0);
            }) `uvm_fatal("MEMORY_RANDOMIZE","Cannot generate a legal memory response")
            if(inject) behavior.injected++;
            if(response.xact_type==svt_axi_transaction::WRITE) begin
                if(behavior.hold_write) begin
                    response.suspend_response=1;
                    behavior.held=response;
                end
                put_write_transaction_data_to_mem(response);
            end else get_read_data_from_mem_to_transaction(response);
            start_item(response); finish_item(response);
        end
    endtask
endclass
