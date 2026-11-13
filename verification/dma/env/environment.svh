class pdma_env extends uvm_env;
    `uvm_component_utils(pdma_env)
    svt_axi_system_env host_bus, memory_bus;
    svt_axi_system_configuration host_cfg;
    svt_axi_reg_adapter register_adapter;
    DmaRegisters registers;
    pdma_configuration cfg;
    pdma_memory memory;
    pdma_memory_policy memory_policy;
    jvs_memory_allocator allocator;
    pdma_clocks clocks;
    jvs_irq_vir_sequencer interrupts;
    pdma_notification notification;
    pdma_channel channels[];
    pdma_device device;
    pdma_irq_handler handlers[];
    pdma_device_irq_handler device_handler;
    pdma_task_checker checkers[];
    pdma_scenario_coverage coverage_;
    uvm_analysis_port#(pdma_observation) observations;
    virtual pdma_control_if control;
    function new(string name,uvm_component parent);
        super.new(name,parent); observations=new("observations",this);
    endfunction
    virtual function void build_phase(uvm_phase phase);
        svt_mem backing;
        jvs_memory_cfg allocation=new("allocation");
        super.build_phase(phase);
        if(!uvm_config_db#(virtual pdma_control_if)::get(this,"","control",control))
            `uvm_fatal("CONTROL_BINDING","Missing test control interface")
        if(!uvm_config_db#(pdma_configuration)::get(this,"","cfg",cfg))
            `uvm_fatal("ENV_CONFIG","Missing test-owned platform configuration")
        if(cfg.channel_count!=`PDMA_CHANNEL_COUNT || !(cfg.channel_count inside {[1:8]}) || cfg.irq_vectors.size()!=cfg.channel_count)
            `uvm_fatal("DUT_PROFILE","Configuration must match the elaborated channel count in 1..8")
        uvm_config_db#(pdma_configuration)::set(this,"memory_bus.slave[0].sequencer","behavior",cfg);
        host_cfg=cfg.system_config(1);
        uvm_config_db#(svt_axi_system_configuration)::set(this,"host_bus","cfg",host_cfg);
        uvm_config_db#(svt_axi_system_configuration)::set(this,"memory_bus","cfg",cfg.system_config(0));
        host_bus=svt_axi_system_env::type_id::create("host_bus",this);
        memory_bus=svt_axi_system_env::type_id::create("memory_bus",this);
        registers=DmaRegisters::type_id::create("registers");
        registers.build(cfg.channel_count,cfg.REGISTER_DATA_BITS/8);
        registers.lock_model();
        register_adapter=svt_axi_reg_adapter::type_id::create("register_adapter");
        register_adapter.p_cfg=host_cfg.master_cfg[0];
        backing=new("memory","AMBA3",cfg.MEMORY_DATA_BITS,0,0,64'hffff_ffff_ffff_ffff);
        svt_config_object_db#(svt_mem)::set(this,"memory_bus.slave*","axi_slave_mem",backing);
        memory=new; memory.backing=backing;
        memory_policy=new; memory_policy.behavior=cfg; memory_policy.memory=memory;
        uvm_config_db#(pdma_memory_policy)::set(this,"memory_bus.slave[0].sequencer","policy",memory_policy);
        allocator=new; allocation.model=memory; allocation.range=new;
        allocation.range.configurate(cfg.heap_base,cfg.heap_size); allocator.configurate(allocation);
        uvm_config_db#(uvm_object_wrapper)::set(this,"memory_bus.slave[0].sequencer.run_phase","default_sequence",pdma_memory_response::type_id::get());
        clocks=pdma_clocks::type_id::create("clocks",this);
        interrupts=jvs_irq_vir_sequencer::type_id::create("interrupts",this);
        notification=pdma_notification::type_id::create("notification",this);
        notification.control=control; notification.sequencer=interrupts;
        channels=new[cfg.channel_count]; handlers=new[cfg.channel_count]; checkers=new[cfg.channel_count];
        foreach(channels[i]) begin
            channels[i]=new($sformatf("channel_%0d",i));
            channels[i].bind_registers(registers,i,cfg.irq_vectors[i]);
            channels[i].allocator=allocator; channels[i].cfg=cfg;
            channels[i].memory=memory; channels[i].control=control;
            handlers[i]=new($sformatf("channel_%0d_handler",i)); handlers[i].channel=channels[i];
            checkers[i]=pdma_task_checker::type_id::create($sformatf("channel_%0d_checker",i));
            checkers[i].channel_id=i;
        end
        notification.channels=channels; notification.device_vector=cfg.device_irq_vector;
        device=new; device.cfg=cfg; device.registers=registers.admin_block.control_block; device.channels=channels;
        device_handler=new("device_fatal_handler"); device_handler.device=device;
        device_handler.vector=cfg.device_irq_vector;
        coverage_=pdma_scenario_coverage::type_id::create("coverage",this);
    endfunction
    virtual function void connect_phase(uvm_phase phase);
        uvm_callbacks#(svt_axi_port_monitor,svt_axi_port_monitor_callback)::add(memory_bus.slave[0].monitor,memory_policy);
        registers.default_map.set_sequencer(host_bus.master[0].sequencer,register_adapter);
        registers.default_map.set_auto_predict(1);
        registers.default_map.set_check_on_read(0);
        observations.connect(coverage_.analysis_export);
    endfunction
    virtual task configure_phase(uvm_phase phase);
        phase.raise_objection(this);
        foreach(channels[i]) interrupts.register_irq(channels[i].vector,handlers[i]);
        interrupts.register_irq(cfg.device_irq_vector,device_handler);
        phase.drop_objection(this);
    endtask
    virtual task main_phase(uvm_phase phase);
        jvs_irq_vir_seq service=new("irq_service"); service.start(interrupts);
    endtask
    function pdma_task_flow task_flow(int channel_id=0);
        pdma_task_flow flow_=pdma_task_flow::type_id::create("task_flow");
        if(channel_id>=channels.size()) `uvm_fatal("CHANNEL_CONFIG","Requested an unimplemented channel")
        flow_.device=device; flow_.channel=channels[channel_id]; flow_.checker_=checkers[channel_id];
        flow_.memory=memory; flow_.allocator=allocator; flow_.observations=observations;
        return flow_;
    endfunction
endclass
