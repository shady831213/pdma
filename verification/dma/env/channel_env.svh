class pdma_channel_env extends uvm_env;
    `uvm_component_utils(pdma_channel_env)
    pdma_channel driver;
    pdma_irq_env #(pdma_channel) irq;
    pdma_task_checker checker_;
    function new(string name,uvm_component parent);
        super.new(name,parent);
    endfunction
    virtual function void build_phase(uvm_phase phase);
        super.build_phase(phase);
        checker_=pdma_task_checker::type_id::create("checker",this);
        checker_.channel_id=driver.channel_id;
        irq=pdma_irq_env#(pdma_channel)::type_id::create("irq",this);
        irq.handler=new(driver);
    endfunction
endclass
