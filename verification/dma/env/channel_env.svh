class pdma_channel_env extends uvm_env;
    `uvm_component_utils(pdma_channel_env)
    pdma_channel driver;
    pdma_task_checker checker_;
    pdma_irq_env irq;
    function new(string name,uvm_component parent);
        super.new(name,parent);
        driver=new(name);
    endfunction
    virtual function void build_phase(uvm_phase phase);
        pdma_irq_handler handler=new("handler");
        super.build_phase(phase);
        checker_=pdma_task_checker::type_id::create("checker",this);
        checker_.channel_id=driver.channel_id;
        handler.channel=driver;
        irq=pdma_irq_env::type_id::create("irq",this);
        irq.vector=driver.vector; irq.handler=handler;
    endfunction
endclass
