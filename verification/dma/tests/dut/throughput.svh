// Keep short/long tasks independently selectable and parallelizable by the runner.
virtual class pdma_throughput_test extends pdma_base_test;
    pdma_performance performance;
    int unsigned transfer_bytes=16384;
    function new(string name,uvm_component parent); super.new(name,parent); endfunction
    virtual function void build_phase(uvm_phase phase);
        super.build_phase(phase);
        platform.delay_max=0; platform.memory_default_ready=1; platform.memory_outstanding=32;
        clocks.select_mode("target");
        clocks.jitter='{0,0,0};
        performance=new("performance",this);
    endfunction
    virtual function void connect_phase(uvm_phase phase);
        super.connect_phase(phase);
        performance.attach(env.memory_bus.slave[0].monitor);
    endfunction
    function int unsigned write_latency_budget();
`ifdef PDMA_CDC_LATENCY_STRESS
        // Only the simulation-only instrumented snapshot has this definition.
        // A receipt may wait one bus cycle behind its peer, rounded at core
        // sampling. Add one injected core cycle, then one injected request-side
        // bus cycle. This WHOLE-task budget is never renewed per burst or derived
        // from measured DUT speed. Ordinary builds always retain zero allowance.
        int core_period=clocks.period_ns[1], bus_period=clocks.period_ns[2];
        int variation=((bus_period+core_period-1)/core_period+1)*core_period;
        return (variation+bus_period-1)/bus_period+1;
`else
        return 0;
`endif
    endfunction
    virtual task run_scenario();
        pdma_task_flow flow_=env.task_flow();
        pdma_task job=settings(transfer_bytes);
        job.payload_alignment=pdma_configuration::AXI_BOUNDARY_BYTES;
        flow_.open(job);
        performance.begin_task(job,clocks.period_ns[2]*1ns,flow_.channel.next_command_address());
        flow_.submit(); flow_.expect_result(TASK_SUCCESS);
        performance.finish(write_latency_budget());
        flow_.close(); cases++;
    endtask
endclass

class pdma_zero_delay_ratio_test extends pdma_throughput_test;
    `uvm_component_utils(pdma_zero_delay_ratio_test)
    function new(string name,uvm_component parent); super.new(name,parent); endfunction
    virtual function void build_phase(uvm_phase phase);
        super.build_phase(phase);
        // Core runs at half the bus frequency; their phase remains independent.
        clocks.period_ns[1]=2;
    endfunction
endclass

class pdma_zero_delay_batch_test extends pdma_throughput_test;
    `uvm_component_utils(pdma_zero_delay_batch_test)
    pdma_performance metrics[4];
    function new(string name,uvm_component parent); super.new(name,parent); expected_cases=4; endfunction
    virtual function void build_phase(uvm_phase phase);
        super.build_phase(phase); platform.command_capacity=16;
        metrics[0]=performance;
        for(int i=1;i<4;i++) metrics[i]=new($sformatf("performance_%0d",i),this);
    endfunction
    virtual function void connect_phase(uvm_phase phase);
        super.connect_phase(phase);
        for(int i=1;i<4;i++) metrics[i].attach(env.memory_bus.slave[0].monitor);
    endfunction
    virtual task run_scenario();
        pdma_task_flow flows[4];
        pdma_channel channel=env.channels[0];
        // Prepublish the whole batch while disabled, so software MMIO latency
        // cannot starve hardware at any measured command boundary.
        channel.write_reg(channel.registers.control_block.config_block.enable_reg,0);
        foreach(flows[i]) begin
            pdma_task job=settings(64);
            job.payload_alignment=pdma_configuration::AXI_BOUNDARY_BYTES;
            flows[i]=env.task_flow(); flows[i].open(job);
            metrics[i].begin_task(job,clocks.period_ns[2]*1ns,channel.next_command_address());
            flows[i].submit();
        end
        channel.write_reg(channel.registers.control_block.config_block.enable_reg,
            1<<channel.registers.control_block.config_block.enable_reg.enabled_field.get_lsb_pos());
        foreach(flows[i]) begin
            flows[i].expect_result(TASK_SUCCESS); metrics[i].finish(write_latency_budget()); cases++;
        end
        `uvm_info("COPY_BATCH",$sformatf("tasks=4 payload_bytes=256 first_descriptor_to_last_fence_cycles=%0.0f",
            (metrics[3].fence_at-metrics[0].descriptor_at)/(clocks.period_ns[2]*1ns)),UVM_NONE)
        foreach(flows[i]) flows[i].close();
    endtask
endclass

class pdma_zero_delay_long_test extends pdma_throughput_test;
    `uvm_component_utils(pdma_zero_delay_long_test)
    function new(string name,uvm_component parent); super.new(name,parent); endfunction
endclass

class pdma_zero_delay_short_test extends pdma_throughput_test;
    `uvm_component_utils(pdma_zero_delay_short_test)
    function new(string name,uvm_component parent);
        super.new(name,parent); transfer_bytes=64;
    endfunction
endclass
