class pdma_memory_response extends svt_axi_slave_base_sequence;
    `uvm_object_utils(pdma_memory_response)
    function new(string name="memory_response"); super.new(name); endfunction
    virtual task body();
        svt_configuration raw;
        svt_axi_slave_transaction response;
        pdma_configuration behavior;
        pdma_memory_policy policy;
        if(!uvm_config_db#(pdma_configuration)::get(p_sequencer,"","behavior",behavior))
            `uvm_fatal("MEMORY_CONFIG","Missing responder configuration")
        if(!uvm_config_db#(pdma_memory_policy)::get(p_sequencer,"","policy",policy))
            `uvm_fatal("MEMORY_CONFIG","Missing address policy")
        p_sequencer.get_cfg(raw);
        if(!$cast(cfg,raw)) `uvm_fatal("MEMORY_CONFIG","Unexpected VIP configuration")
        instantiate_axi_slave_mem();
        sink_responses();
        forever begin
            p_sequencer.response_request_port.peek(response);
            if(!response.randomize() with {
                bresp==svt_axi_transaction::OKAY;
                foreach(rresp[i]) rresp[i]==svt_axi_transaction::OKAY;
                if(xact_type==svt_axi_transaction::WRITE && local::behavior.write_address_delay>=0)
                    addr_ready_delay==local::behavior.write_address_delay;
                else addr_ready_delay inside {[0:local::behavior.delay_max]};
                bvalid_delay inside {[0:local::behavior.delay_max]};
                foreach(wready_delay[i])
                    if(local::behavior.write_data_ready_delay>=0)
                        wready_delay[i]==local::behavior.write_data_ready_delay;
                    else wready_delay[i] inside {[0:local::behavior.delay_max]};
                foreach(rvalid_delay[i]) rvalid_delay[i] inside {[0:local::behavior.delay_max]};
                enable_interleave==0;
                resp_user==0;
                foreach(data_user[i]) data_user[i]==0;
            }) `uvm_fatal("MEMORY_RANDOMIZE","Cannot generate a legal memory response")
            response.suspend_response=0;
            if(response.xact_type==svt_axi_transaction::WRITE) begin
                if(behavior.hold_write) begin
                    response.suspend_response=1;
                    behavior.held.push_back(response);
                end
                put_write_transaction_data_to_mem(response);
            end else begin
                policy.apply_access(response);
                get_read_data_from_mem_to_transaction(response);
            end
            start_item(response); finish_item(response);
        end
    endtask
endclass
