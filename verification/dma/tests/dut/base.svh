virtual class pdma_base_test extends uvm_test;
    protected int cases;
    protected bit interrupt_driven;
    protected int expected_cases=1;
    pdma_env env;
    pdma_configuration platform;
    pdma_clocks_config clocks;
    function new(string name,uvm_component parent); super.new(name,parent); endfunction
    virtual function void build_phase(uvm_phase phase);
        string clock_mode="random", mutation;
        platform=new; clocks=new;
        platform.ring_context=$urandom;
        void'($value$plusargs("clock_mode=%s",clock_mode));
        clocks.select_mode(clock_mode);
        platform.irq_timeout_ns=4000*clocks.period_ns[1];
        if($value$plusargs("checker_mutation=%s",mutation)) begin
            if(!(mutation inside {"data","identity","unknown_identity","missing_completion","duplicate_completion",
                "early_fence","fatal_fence","underreported_prefix","fractional_prefix"}))
                `uvm_fatal("MUTATION_CONFIG","Unknown mutation")
            pdma_task_checker::type_id::set_type_override(pdma_negative_checker::get_type());
            if(mutation=="missing_completion") pdma_task_flow::type_id::set_type_override(pdma_missing_fence::get_type());
        end
        uvm_config_db#(pdma_configuration)::set(this,"env","cfg",platform);
        uvm_config_db#(pdma_clocks_config)::set(this,"env.clocks","cfg",clocks);
        env=pdma_env::type_id::create("env",this);
    endfunction
    function pdma_task settings(int requested_length=64,bit varied=0);
        pdma_task job=new;
        if(requested_length>job.max_length) job.max_length=requested_length;
        if(!job.randomize() with {
            local::requested_length==0 || length==local::requested_length;
            if(!local::varied) {source_lane==0; dest_lane==0; cross_page==0;}
        }) `uvm_fatal("TASK_CONFIG","Cannot configure task")
        return job;
    endfunction
    // Fault/replay scenarios mutate the source after a failed write. Every
    // byte they mutate must belong to the single burst retained by the DUT.
    function pdma_task single_burst_copy();
        int unsigned target=pdma_configuration::MEMORY_BEAT_BYTES>64 ? pdma_configuration::MEMORY_BEAT_BYTES : 64;
        pdma_task job=settings(target<pdma_configuration::COPY_BURST_BYTES ? target : pdma_configuration::COPY_BURST_BYTES);
        job.payload_alignment=1<<$clog2(job.length);
        return job;
    endfunction
    task inject(bit wr,bit [1:0] reason,longint unsigned address);
        platform.fault_enabled=1; platform.fault_write=wr;
        platform.fault_reason=reason; platform.fault_address=address;
    endtask
    pure virtual task run_scenario();
    virtual task main_phase(uvm_phase phase);
        phase.raise_objection(this);
        env.device.driver.enable_notifications();
        foreach(env.device.driver.channels[i]) begin
            env.device.driver.channels[i].interrupt_driven=interrupt_driven;
            env.device.driver.channels[i].setup();
        end
        run_scenario();
        foreach(env.device.driver.channels[i]) begin
            pdma_channel channel=env.device.driver.channels[i];
            if(channel.configured) begin
                // The scenario has joined all flows. These belong to the channel,
                // so a later command's status is never attributed to an earlier Fence.
                uvm_reg_data_t cause, producer;
                channel.read_reg(channel.registers.control_block.irq_block.cause_reg,cause);
                channel.read_reg(channel.registers.event_block.producer_reg,producer);
                if(cause!==0 || producer!==channel.event_consumer)
                    `uvm_fatal("CHANNEL_QUIESCENCE","Unexpected IRQ cause or unconsumed event at teardown")
            end
            channel.teardown();
        end
        phase.drop_objection(this);
    endtask
    virtual function void check_phase(uvm_phase phase);
        int checked_tasks=0;
        foreach(env.device.channels[i]) checked_tasks+=env.device.channels[i].checker_.checked_tasks;
        if(cases!=expected_cases || checked_tasks<expected_cases)
            `uvm_error("TEST_INCOMPLETE","Required scenarios or task checks are missing")
    endfunction
    virtual function void report_phase(uvm_phase phase);
        uvm_report_server reports=uvm_report_server::get_server();
        int checked_tasks=0;
        longint unsigned checked_bytes=0;
        foreach(env.device.channels[i]) begin
            checked_tasks+=env.device.channels[i].checker_.checked_tasks;
            checked_bytes+=env.device.channels[i].checker_.checked_bytes;
        end
        if(cases==expected_cases && reports.get_severity_count(UVM_ERROR)==0 && reports.get_severity_count(UVM_FATAL)==0) begin
            `uvm_info("TEST_RESULT",$sformatf("test=%s scenarios=%0d checked_tasks=%0d checked_bytes=%0d",get_type_name(),cases,
                checked_tasks,checked_bytes),UVM_NONE)
            $display("PDMA_VIP_PASS test=%s",get_type_name());
        end else `uvm_fatal("TEST_FAILED","Incomplete or failing test")
    endfunction
endclass
