virtual class pdma_base_test extends uvm_test;
    protected int cases;
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
        if($value$plusargs("checker_mutation=%s",mutation)) begin
            if(!(mutation inside {"data","identity","unknown_identity","missing_completion","duplicate_completion",
                "early_fence","underreported_prefix","fractional_prefix"}))
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
    task inject(bit wr,bit page,longint unsigned address);
        platform.fault_enabled=1; platform.fault_write=wr;
        platform.fault_page=page; platform.fault_address=address;
    endtask
    pure virtual task run_scenario();
    virtual task main_phase(uvm_phase phase);
        phase.raise_objection(this);
        env.device.enable_notifications();
        foreach(env.channels[i]) env.channels[i].setup();
        run_scenario();
        foreach(env.channels[i]) env.channels[i].teardown();
        phase.drop_objection(this);
    endtask
    virtual function void check_phase(uvm_phase phase);
        int checked_tasks=0;
        foreach(env.checkers[i]) checked_tasks+=env.checkers[i].checked_tasks;
        if(cases!=expected_cases || checked_tasks<expected_cases)
            `uvm_error("TEST_INCOMPLETE","Required scenarios or task checks are missing")
    endfunction
    virtual function void report_phase(uvm_phase phase);
        uvm_report_server reports=uvm_report_server::get_server();
        int checked_tasks=0;
        longint unsigned checked_bytes=0;
        foreach(env.checkers[i]) begin
            checked_tasks+=env.checkers[i].checked_tasks;
            checked_bytes+=env.checkers[i].checked_bytes;
        end
        if(cases==expected_cases && reports.get_severity_count(UVM_ERROR)==0 && reports.get_severity_count(UVM_FATAL)==0) begin
            `uvm_info("TEST_RESULT",$sformatf("test=%s scenarios=%0d checked_tasks=%0d checked_bytes=%0d",get_type_name(),cases,
                checked_tasks,checked_bytes),UVM_NONE)
            $display("PDMA_VIP_PASS test=%s",get_type_name());
        end else `uvm_fatal("TEST_FAILED","Incomplete or failing test")
    endfunction
endclass
