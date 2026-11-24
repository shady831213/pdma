class pdma_register_model_test extends pdma_base_test;
    `uvm_component_utils(pdma_register_model_test)
    function new(string name,uvm_component parent); super.new(name,parent); endfunction
    virtual task run_scenario();
        pdma_task_flow flow_=env.device.task_flow();
        pdma_channel channel=flow_.channel;
        uvm_reg_data_t observed;
        uvm_reg_data_t pointer_value=64'h12345678_abcde000;
        uvm_status_e status;
        uvm_reg_addr_t original_base=env.device.registers.default_map.get_base_addr();
        channel.request_reset(); channel.finish_reset();
        channel.write_reg(channel.registers.command_block.base_reg,pointer_value);
        channel.read_reg(channel.registers.command_block.base_reg,observed);
        if(observed!==pointer_value)
            `uvm_fatal("RAL_REGISTER_ACCESS","64-bit register did not round-trip through the configured bus")
        env.device.registers.default_map.set_base_addr('h40000000);
        env.device.registers.discovery_block.channel_num_reg.read(status,observed,UVM_FRONTDOOR);
        if(status!=UVM_NOT_OK) `uvm_fatal("RAL_BUS_ERROR","Unmapped register access did not propagate failure")
        env.device.registers.default_map.set_base_addr(original_base);
        channel.configure();
        flow_.open(settings(32)); flow_.submit(); flow_.expect_fence(); flow_.close();
        cases++;
    endtask
endclass

class pdma_ring_context_lock_test extends pdma_base_test;
    `uvm_component_utils(pdma_ring_context_lock_test)
    function new(string name,uvm_component parent); super.new(name,parent); endfunction
    virtual function void build_phase(uvm_phase phase);
        super.build_phase(phase); expected_cases=platform.channel_count;
    endfunction
    virtual task run_scenario();
        bit[31:0] original=platform.ring_context;
        bit[31:0] replacement=original=='1 ? 1 : ~original;
        foreach(env.device.driver.channels[i]) begin
            pdma_channel ch=env.device.driver.channels[i];
            uvm_reg selector=ch.registers.control_block.config_block.ring_context_reg;
            uvm_reg_data_t observed;
            uvm_status_e status;
            // Disabling command processing alone does not release the binding.
            for(int pass=0;pass<2;pass++) begin
                if(pass==1) ch.write_reg(ch.registers.control_block.config_block.enable_reg,0);
                selector.write(status,replacement,UVM_FRONTDOOR);
                if(status!==UVM_NOT_OK)
                    `uvm_fatal("RING_CONTEXT_LOCK",$sformatf("Channel %0d accepted a locked context write",i))
                ch.read_reg(selector,observed);
                if(observed!==original)
                    `uvm_fatal("RING_CONTEXT_LOCK","Rejected write changed the ring context")
            end
            ch.teardown();
            ch.read_reg(selector,observed);
            if(observed!==0) `uvm_fatal("RING_CONTEXT_RESET","Completed reset retained the context")
        end
        platform.ring_context=replacement;
        foreach(env.device.driver.channels[i]) begin
            pdma_task_flow flow_=env.device.task_flow(i);
            uvm_reg_data_t observed;
            flow_.channel.setup();
            flow_.channel.read_reg(flow_.channel.registers.control_block.config_block.ring_context_reg,observed);
            if(observed!==replacement) `uvm_fatal("RING_CONTEXT_REBIND","New context did not take effect")
            // Newly allocated rings permit DMA accesses only under the new selector.
            flow_.open(settings()); flow_.submit(); flow_.expect_fence(); flow_.close(); cases++;
        end
    endtask
endclass
