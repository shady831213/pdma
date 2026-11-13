virtual class pdma_reset_test extends pdma_base_test;
    protected bit global_scope;
    function new(string name,uvm_component parent); super.new(name,parent); endfunction
    virtual task run_scenario();
        pdma_task_flow flow_=env.task_flow();
        pdma_task job=settings(128);
        pdma_channel channel=flow_.channel;
        Reset reset_reg=global_scope ? env.device.registers.reset_reg : channel.admin.reset_reg;
        bit[31:0] busy;
        flow_.open(job); platform.hold_write=1; platform.held.delete(); flow_.submit();
        wait(platform.held.size()!=0);
        if(global_scope) env.device.request_reset();
        else channel.request_reset();
        channel.read_reg(reset_reg,busy);
        if(!busy[reset_reg.request_field.get_lsb_pos()])
            `uvm_fatal("RESET_DRAIN","Reset acknowledged before outstanding response")
        platform.hold_write=0;
        foreach(platform.held[i]) platform.held[i].suspend_response=0;
        platform.held.delete();
        flow_.cancel(global_scope);
        job=settings(16); flow_.open(job); flow_.submit(); flow_.expect_result(TASK_SUCCESS); flow_.close(); cases++;
    endtask
endclass

class pdma_channel_reset_test extends pdma_reset_test;
    `uvm_component_utils(pdma_channel_reset_test)
    function new(string name,uvm_component parent);
        super.new(name,parent); global_scope=0;
    endfunction
endclass

class pdma_dma_reset_test extends pdma_reset_test;
    `uvm_component_utils(pdma_dma_reset_test)
    function new(string name,uvm_component parent);
        super.new(name,parent); global_scope=1;
    endfunction
endclass

class pdma_global_reset_peer_test extends pdma_reset_test;
    `uvm_component_utils(pdma_global_reset_peer_test)
    function new(string name,uvm_component parent);
        super.new(name,parent); global_scope=1;
    endfunction
    virtual function void build_phase(uvm_phase phase);
        super.build_phase(phase); expected_cases=platform.channel_count;
    endfunction
    virtual task run_scenario();
        super.run_scenario();
        for(int i=1;i<platform.channel_count;i++) begin
            pdma_task_flow flow_=env.task_flow(i);
            uvm_reg_data_t enabled;
            flow_.open(settings(64));
            flow_.channel.read_reg(flow_.channel.registers.control_block.config_block.enable_reg,enabled);
            if(!enabled[flow_.channel.registers.control_block.config_block.enable_reg.enabled_field.get_lsb_pos()])
                `uvm_fatal("GLOBAL_RESET_RECONFIGURATION","Fresh peer work retained stale pre-reset software configuration")
            flow_.submit(); flow_.expect_result(TASK_SUCCESS); flow_.close(); cases++;
        end
    endtask
endclass

class pdma_global_irq_owner_test extends pdma_base_test;
    `uvm_component_utils(pdma_global_irq_owner_test)
    function new(string name,uvm_component parent); super.new(name,parent); endfunction
    virtual task run_scenario();
        pdma_task_flow flow_=env.task_flow(platform.channel_count-1);
        pdma_channel channel=flow_.channel;
        uvm_reg_data_t mask;
        env.device.write_reg(env.device.registers.irq_enable_reg,0);
        channel.request_reset(); channel.finish_reset();
        flow_.open(settings(64));
        env.device.read_reg(env.device.registers.irq_enable_reg,mask);
        if(mask!=0) `uvm_fatal("GLOBAL_IRQ_OWNER","Channel reconfiguration changed the device notification mask")
        flow_.submit(); flow_.expect_result(TASK_SUCCESS); flow_.close(); cases++;
    endtask
endclass
