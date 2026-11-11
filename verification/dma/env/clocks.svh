class pdma_clock_config extends jvs_root_clk_cfg;
    `uvm_object_utils(pdma_clock_config)
    function new(string name="clock_config"); super.new(name); endfunction
    // Periods use the package time unit; frequencies are Hz. The upstream
    // transaction halves get_period() again when scheduling each clock edge.
    virtual function realtime get_period(); return period*timescale; endfunction
    virtual function realtime get_max_period();
        return get_period()*(1.0+real'(jitter)/255.0);
    endfunction
    virtual function realtime get_min_period();
        return get_period()*(1.0-real'(jitter)/255.0);
    endfunction
    virtual function real get_freq(); return 1s/get_period(); endfunction
    virtual function real get_max_freq(); return 1s/get_min_period(); endfunction
    virtual function real get_min_freq(); return 1s/get_max_period(); endfunction
    virtual function realtime get_init_delay();
        return (real'(init_delay)/255.0)*get_period()/2.0;
    endfunction
endclass

class pdma_clocks_config extends uvm_object;
    `uvm_object_utils(pdma_clocks_config)
    // Array indices are reg, core, bus. Only the test chooses randomization.
    rand int unsigned period_ns[3];
    rand int unsigned jitter[3];
    rand int unsigned initial_phase[3];
    rand int unsigned reset_cycles[3];
    constraint valid_clocks {
        period_ns[0] inside {[9:15]};
        period_ns[1] inside {[5:17]};
        period_ns[2] inside {[4:11]};
        foreach(jitter[i]) {
            jitter[i] inside {[0:63]};
            initial_phase[i] inside {[1:255]};
            reset_cycles[i] inside {[5:12]};
        }
    }
    function new(string name="clocks_config");
        super.new(name);
        period_ns='{10,6,14};
        jitter='{0,0,0};
        initial_phase='{64,128,192};
        reset_cycles='{8,8,8};
    endfunction
    static function string domain_name(int index);
        case(index)
            0: return "reg";
            1: return "core";
            2: return "bus";
            default: begin
                `uvm_fatal("CLOCK_DOMAIN","Invalid clock domain index")
                return "";
            end
        endcase
    endfunction
    function jvs_clk_top_cfg make_config();
        jvs_clk_top_cfg result=new("clocks");
        foreach(period_ns[i]) begin
            jvs_clk_group_cfg group_cfg=new(domain_name(i));
            pdma_clock_config root_cfg=new("root");
            jvs_gen_clk_cfg gen_cfg=new("clock");
            if(period_ns[i]==0 || jitter[i]>63 || initial_phase[i]<1 ||
                initial_phase[i]>255 || reset_cycles[i]<1 || reset_cycles[i]>255)
                `uvm_fatal("CLOCK_CONFIG",$sformatf("Invalid %s clock configuration",domain_name(i)))
            root_cfg.timescale=1ns;
            root_cfg.period=period_ns[i];
            root_cfg.jitter=jitter[i];
            root_cfg.init_delay=initial_phase[i];
            group_cfg.root_clk=root_cfg;
            gen_cfg.div_ratio=1;
            gen_cfg.sync_rst=0;
            gen_cfg.rst_cycle=reset_cycles[i];
            group_cfg.add_gen_clk(gen_cfg);
            result.add_group(group_cfg);
        end
        return result;
    endfunction
endclass

class pdma_clocks extends uvm_component;
    `uvm_component_utils(pdma_clocks)
    pdma_clocks_config cfg;
    jvs_clk_top_cfg driver_cfg;
    jvs_clk_env driver;
    virtual jvs_gen_clk_if vif[3];
    function new(string name,uvm_component parent); super.new(name,parent); endfunction
    virtual function void build_phase(uvm_phase phase);
        super.build_phase(phase);
        if(!uvm_config_db#(pdma_clocks_config)::get(this,"","cfg",cfg))
            `uvm_fatal("CLOCK_CONFIG","Missing test-owned clock configuration")
        foreach(vif[i]) begin
            if(!uvm_config_db#(virtual jvs_gen_clk_if)::get(this,"",cfg.domain_name(i),vif[i]))
                `uvm_fatal("CLOCK_BINDING",$sformatf("Missing %s generated clock interface",cfg.domain_name(i)))
        end
        driver_cfg=cfg.make_config();
        uvm_config_db#(jvs_clk_top_cfg)::set(this,"driver","cfg",driver_cfg);
        driver=jvs_clk_env::type_id::create("driver",this);
    endfunction
    function realtime watchdog_delay();
        realtime delay=0;
        foreach(cfg.period_ns[i])
            delay+=(cfg.reset_cycles[i]+128)*driver_cfg.groups[cfg.domain_name(i)].root_clk.get_max_period();
        return delay;
    endfunction
    // Completion synchronization only; stimulus edge/period checks belong to TB selftests.
    task await_reset(int index);
        wait(vif[index].rst_process===1'b1);
        wait(vif[index].reset_n===1'b0);
        @(posedge vif[index].reset_n);
        wait(vif[index].rst_process===1'b0);
        if(vif[index].reset_n!==1'b1)
            `uvm_fatal("CLOCK_RESET",$sformatf("%s reset remained asserted",cfg.domain_name(index)))
    endtask
    virtual task reset_phase(uvm_phase phase);
        phase.raise_objection(this);
        // The wrapper process owns only this reset operation and its watchdog.
        fork begin
            fork
                begin
                    // Group event names are shared upstream; wait on every real interface.
                    fork
                        driver.hw_reset("*");
                        await_reset(0);
                        await_reset(1);
                        await_reset(2);
                    join
                end
                begin
                    #(watchdog_delay());
                    `uvm_fatal("CLOCK_RESET_TIMEOUT","Power-up clock reset did not complete")
                end
            join_any
            disable fork;
        end join
        phase.drop_objection(this);
    endtask
endclass
