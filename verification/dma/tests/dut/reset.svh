virtual class pdma_reset_test extends pdma_base_test;
    protected bit global_scope;
    function new(string name,uvm_component parent); super.new(name,parent); endfunction
    virtual task run_scenario();
        pdma_task_flow flow_=env.task_flow();
        pdma_task job=settings(128);
        pdma_channel channel=flow_.channel;
        Reset reset_reg=global_scope ? channel.dma_control.reset_reg : channel.admin.reset_reg;
        bit[31:0] busy;
        flow_.open(job); platform.hold_write=1; platform.held.delete(); flow_.submit();
        wait(platform.held.size()!=0);
        channel.request_reset(global_scope);
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
