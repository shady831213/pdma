virtual class pdma_reset_test extends pdma_base_test;
    protected bit global_scope;
    function new(string name,uvm_component parent); super.new(name,parent); endfunction
    virtual task run_scenario();
        pdma_task_flow flow_=env.task_flow();
        pdma_task job=settings(128);
        pdma_channel channel=flow_.channel;
        bit[31:0] busy;
        flow_.open(job); platform.hold_write=1; platform.held=null; flow_.submit();
        wait(platform.held!=null);
        channel.request_reset(global_scope);
        channel.read_reg(global_scope ? channel.global_base+pdma_rtl_DmaControlLayout::RESET_OFFSET : channel.reset_address,busy);
        if(!busy[0]) `uvm_fatal("RESET_DRAIN","Reset acknowledged before outstanding response")
        platform.hold_write=0; platform.held.suspend_response=0;
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
