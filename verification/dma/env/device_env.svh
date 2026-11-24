class pdma_device_env extends uvm_env;
    `uvm_component_utils(pdma_device_env)
    pdma_configuration cfg;
    pdma_memory memory;
    jvs_memory_allocator allocator;
    DmaRegisters registers;
    pdma_device driver;
    pdma_irq_env #(pdma_device) fatal_irq;
    pdma_channel_env channels[];
    uvm_analysis_port#(pdma_observation) observations;

    function new(string name,uvm_component parent);
        super.new(name,parent);
        observations=new("observations",this);
    endfunction

    virtual function void build_phase(uvm_phase phase);
        super.build_phase(phase);
        if(cfg.channel_count!=`PDMA_CHANNEL_COUNT || !(cfg.channel_count inside {[1:8]}))
            `uvm_fatal("DUT_PROFILE","Configuration must match the elaborated channel count in 1..8")
        registers=DmaRegisters::type_id::create("registers");
        registers.build(cfg.channel_count,cfg.REGISTER_WORD_BYTES);
        registers.lock_model();
        driver=new(cfg,registers,memory,allocator);
        channels=new[driver.channels.size()];
        foreach(channels[i]) begin
            channels[i]=pdma_channel_env::type_id::create($sformatf("channel_%0d",i),this);
            channels[i].driver=driver.channels[i];
        end
        fatal_irq=pdma_irq_env#(pdma_device)::type_id::create("fatal_irq",this);
        fatal_irq.handler=new(driver);
    endfunction

    function pdma_task_flow task_flow(int channel_id=0);
        pdma_task_flow flow_=pdma_task_flow::type_id::create("task_flow");
        if(channel_id<0 || channel_id>=channels.size()) `uvm_fatal("CHANNEL_CONFIG","Requested an unimplemented channel")
        flow_.device=driver;
        flow_.channel=channels[channel_id].driver;
        flow_.irq_handler=channels[channel_id].irq.handler;
        flow_.checker_=channels[channel_id].checker_;
        flow_.memory=memory; flow_.allocator=allocator; flow_.observations=observations;
        return flow_;
    endfunction
endclass
