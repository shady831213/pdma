// Selected only by pdma_clock_selftest; ordinary DUT tests create no such observer.
class pdma_checked_clocks extends pdma_clocks;
    `uvm_component_utils(pdma_checked_clocks)
    int unsigned period_samples[3]='{default:0};
    int unsigned reset_samples[3]='{default:0};
    function new(string name,uvm_component parent); super.new(name,parent); endfunction
    task check_reset(int index);
        realtime last_rise=-1;
        fork begin
            fork
                forever begin @(posedge vif[index].clock); last_rise=$realtime; end
                begin
                    wait(vif[index].rst_process===1'b1);
                    wait(vif[index].reset_n===1'b0);
                    @(posedge vif[index].reset_n);
                    if(vif[index].clock!==1'b1 || $realtime!=last_rise)
                        `uvm_fatal("CLOCK_RESET_SYNC",$sformatf("%s reset did not release on its own edge",cfg.domain_name(index)))
                    wait(vif[index].rst_process===1'b0);
                    reset_samples[index]++;
                end
            join_any
            disable fork;
        end join
    endtask
    virtual task reset_phase(uvm_phase phase);
        phase.raise_objection(this);
        fork begin
            fork
                begin
                    fork
                        super.reset_phase(phase);
                        check_reset(0);
                        check_reset(1);
                        check_reset(2);
                    join
                end
                begin #(watchdog_delay()*2.0); `uvm_fatal("TB_RESET_TIMEOUT","Reset selftest did not finish") end
            join_any
            disable fork;
        end join
        phase.drop_objection(this);
    endtask
    task check_period(int index);
        realtime previous, delta;
        // Oracle uses requested units, not the adapter's get_period()/get_*_period().
        realtime requested=cfg.period_ns[index]*1ns;
        realtime minimum=requested*(1.0-real'(cfg.jitter[index])/255.0);
        realtime maximum=requested*(1.0+real'(cfg.jitter[index])/255.0);
        @(posedge vif[index].clock); previous=$realtime;
        repeat(64) begin
            @(posedge vif[index].clock);
            delta=$realtime-previous;
            if(delta<minimum-0.002ns || delta>maximum+0.002ns)
                `uvm_fatal("CLOCK_PERIOD",$sformatf("%s observed=%0t bounds=[%0t,%0t]",cfg.domain_name(index),delta,minimum,maximum))
            previous=$realtime;
            period_samples[index]++;
        end
        `uvm_info("CLOCK_CHECK",$sformatf("%s verified_periods=%0d period_ns=%0d jitter=%0d phase=%0d",cfg.domain_name(index),
            period_samples[index],cfg.period_ns[index],cfg.jitter[index],cfg.initial_phase[index]),UVM_LOW)
    endtask
    virtual task post_reset_phase(uvm_phase phase);
        phase.raise_objection(this);
        fork begin
            fork
                begin fork check_period(0); check_period(1); check_period(2); join end
                begin #(watchdog_delay()*2.0); `uvm_fatal("TB_CLOCK_TIMEOUT","Clock selftest did not finish") end
            join_any
            disable fork;
        end join
        phase.drop_objection(this);
    endtask
endclass
class pdma_clock_selftest extends pdma_tb_test;
    `uvm_component_utils(pdma_clock_selftest)
    function new(string name,uvm_component parent); super.new(name,parent); endfunction
    virtual function void build_phase(uvm_phase phase);
        pdma_clocks::type_id::set_type_override(pdma_checked_clocks::get_type());
        super.build_phase(phase);
    endfunction
    virtual task check_tb();
        pdma_checked_clocks probe;
        if(!$cast(probe,clocks)) `uvm_fatal("TB_CLOCK_MISSING","Clock selftest observer was not installed")
        foreach(probe.period_samples[i])
            if(probe.period_samples[i]!=64 || probe.reset_samples[i]!=1)
                `uvm_fatal("TB_CLOCK_INCOMPLETE","Missing clock or reset samples")
    endtask
endclass
