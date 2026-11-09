class pdma_register_model_test extends pdma_base_test;
    `uvm_component_utils(pdma_register_model_test)
    function new(string name,uvm_component parent); super.new(name,parent); endfunction
    virtual task run_scenario();
        pdma_task_flow flow_=env.task_flow();
        pdma_channel channel=flow_.channel;
        uvm_reg_data_t observed;
        uvm_reg_data_t pointer_value=64'h12345678_abcde000;
        uvm_status_e status;
        uvm_reg_addr_t original_base=env.registers.default_map.get_base_addr();
        channel.request_reset(); channel.finish_reset();
        channel.write_reg(channel.registers.command_block.base_reg,pointer_value);
        channel.read_reg(channel.registers.command_block.base_reg,observed);
        if(observed!==pointer_value)
            `uvm_fatal("RAL_SPLIT_ACCESS","64-bit register did not round-trip through the 32-bit bus")
        env.registers.default_map.set_base_addr('h40000000);
        env.registers.discovery_block.channel_num_reg.read(status,observed,UVM_FRONTDOOR);
        if(status!=UVM_NOT_OK) `uvm_fatal("RAL_BUS_ERROR","Unmapped register access did not propagate failure")
        env.registers.default_map.set_base_addr(original_base);
        channel.configure();
        flow_.open(settings(32)); flow_.submit(); flow_.expect_result(TASK_SUCCESS); flow_.close();
        cases++;
    endtask
endclass
