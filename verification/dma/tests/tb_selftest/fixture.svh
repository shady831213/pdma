// Minimal clocked fixture: no DMA software flow, checker or functional coverage.
// Preserve the top's env.clocks interface binding without constructing a DMA env.
class pdma_tb_fixture extends uvm_env;
    function new(string name,uvm_component parent); super.new(name,parent); endfunction
endclass
virtual class pdma_tb_test extends uvm_test;
    pdma_tb_fixture env;
    pdma_clocks clocks;
    pdma_clocks_config clock_cfg;
    virtual pdma_control_if control;
    bit verified;
    function new(string name,uvm_component parent); super.new(name,parent); endfunction
    virtual function void build_phase(uvm_phase phase);
        string mode="random";
        super.build_phase(phase);
        clock_cfg=new;
        void'($value$plusargs("clock_mode=%s",mode));
        case(mode)
            "random": if(!clock_cfg.randomize() with {foreach(jitter[i]) jitter[i] inside {[0:12]};})
                `uvm_fatal("CLOCK_CONFIG","Cannot randomize TB clock configuration")
            "fixed": ;
            default: `uvm_fatal("CLOCK_CONFIG","clock_mode must be fixed or random")
        endcase
        uvm_config_db#(pdma_clocks_config)::set(this,"env.clocks","cfg",clock_cfg);
        env=new("env",this);
        clocks=pdma_clocks::type_id::create("clocks",env);
        if(!uvm_config_db#(virtual pdma_control_if)::get(this,"env","control",control))
            `uvm_fatal("CONTROL_BINDING","Missing selftest control interface")
    endfunction
    function pdma_channel make_channel(string name);
        pdma_channel channel=new(name);
        channel.cfg=new;
        channel.control=control;
        return channel;
    endfunction
    pure virtual task check_tb();
    virtual task main_phase(uvm_phase phase);
        phase.raise_objection(this);
        // The dedicated process owns only this check and its watchdog.
        fork begin
            fork
                begin check_tb(); verified=1; end
                begin #(clocks.watchdog_delay()*4.0); `uvm_fatal("TB_TIMEOUT","Selftest did not finish") end
            join_any
            disable fork;
        end join
        phase.drop_objection(this);
    endtask
    virtual function void report_phase(uvm_phase phase);
        uvm_report_server reports=uvm_report_server::get_server();
        if(!verified || reports.get_severity_count(UVM_ERROR)!=0 || reports.get_severity_count(UVM_FATAL)!=0)
            `uvm_fatal("TB_SELFTEST_FAILED","Missing or failing selftest")
        $display("PDMA_TB_SELFTEST_PASS test=%s",get_type_name());
        // Reuse the native per-case runner; these tests are not in the DUT regression list.
        $display("PDMA_VIP_PASS test=%s",get_type_name());
    endfunction
endclass
