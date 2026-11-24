class pdma_env extends uvm_env;
    `uvm_component_utils(pdma_env)
    svt_axi_system_env host_bus, memory_bus;
    svt_axi_system_configuration host_cfg;
    pdma_register_adapter register_adapter;
    pdma_configuration cfg;
    pdma_memory memory;
    pdma_memory_policy memory_policy;
    jvs_memory_allocator allocator;
    pdma_clocks clocks;
    pdma_device_env device;
    pdma_scenario_coverage coverage_;
    virtual pdma_control_if control;
    function new(string name,uvm_component parent);
        super.new(name,parent);
    endfunction
    virtual function void build_phase(uvm_phase phase);
        svt_mem backing;
        jvs_memory_cfg allocation=new("allocation");
        super.build_phase(phase);
        if(!uvm_config_db#(virtual pdma_control_if)::get(this,"","control",control))
            `uvm_fatal("CONTROL_BINDING","Missing test control interface")
        if(!uvm_config_db#(pdma_configuration)::get(this,"","cfg",cfg))
            `uvm_fatal("ENV_CONFIG","Missing test-owned platform configuration")
        uvm_config_db#(pdma_configuration)::set(this,"memory_bus.slave[0].sequencer","behavior",cfg);
        host_cfg=cfg.system_config(1);
        uvm_config_db#(svt_axi_system_configuration)::set(this,"host_bus","cfg",host_cfg);
        uvm_config_db#(svt_axi_system_configuration)::set(this,"memory_bus","cfg",cfg.system_config(0));
        host_bus=svt_axi_system_env::type_id::create("host_bus",this);
        memory_bus=svt_axi_system_env::type_id::create("memory_bus",this);
        register_adapter=pdma_register_adapter::type_id::create("register_adapter");
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
        device=pdma_device_env::type_id::create("device",this);
        device.cfg=cfg; device.memory=memory;
        device.allocator=allocator;
        coverage_=pdma_scenario_coverage::type_id::create("coverage",this);
    endfunction
    virtual function void connect_phase(uvm_phase phase);
        uvm_callbacks#(svt_axi_port_monitor,svt_axi_port_monitor_callback)::add(memory_bus.slave[0].monitor,memory_policy);
        device.registers.default_map.set_sequencer(host_bus.master[0].sequencer,register_adapter);
        device.registers.default_map.set_auto_predict(1);
        device.registers.default_map.set_check_on_read(0);
        device.observations.connect(coverage_.analysis_export);
    endfunction
endclass
