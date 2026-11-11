class pdma_irq_wait_isolation_test extends pdma_tb_test;
    `uvm_component_utils(pdma_irq_wait_isolation_test)
    function new(string name,uvm_component parent); super.new(name,parent); endfunction
    virtual task check_tb();
        pdma_channel channel=make_channel("channel");
        pdma_result expected=new, actual;
        bit unrelated_done=0;
        fork
            begin repeat(12) @(posedge control.core_clk); unrelated_done=1; end
            begin repeat(2) @(posedge control.core_clk); channel.interrupts.put(expected); end
        join_none
        channel.wait_interrupt(actual);
        if(actual!=expected) `uvm_fatal("IRQ_IDENTITY","Wait returned another notification")
        repeat(16) @(posedge control.core_clk);
        if(!unrelated_done) `uvm_fatal("IRQ_WAIT_SCOPE","Wait killed an unrelated caller-owned thread")
    endtask
endclass

class pdma_expected_irq_timeout extends uvm_report_catcher;
    int caught=0;
    function new(string name="expected_irq_timeout"); super.new(name); endfunction
    virtual function action_e catch();
        if(get_severity()==UVM_FATAL && get_id()=="TASK_TIMEOUT") begin caught++; return CAUGHT; end
        return THROW;
    endfunction
endclass
class pdma_irq_wait_timeout_test extends pdma_tb_test;
    `uvm_component_utils(pdma_irq_wait_timeout_test)
    function new(string name,uvm_component parent); super.new(name,parent); endfunction
    virtual task check_tb();
        pdma_channel channel=make_channel("channel");
        pdma_expected_irq_timeout catcher=new;
        pdma_result late=new, actual, remaining;
        bit unrelated_done=0;
        channel.cfg.irq_timeout_cycles=4;
        uvm_report_cb::add(null,catcher);
        fork begin repeat(8) @(posedge control.core_clk); unrelated_done=1; end join_none
        channel.wait_interrupt(actual);
        uvm_report_cb::delete(null,catcher);
        if(catcher.caught!=1 || actual!=null) `uvm_fatal("IRQ_TIMEOUT","Expected exactly one timeout without a result")
        channel.interrupts.put(late);
        repeat(12) @(posedge control.core_clk);
        if(!channel.interrupts.try_get(remaining) || remaining!=late)
            `uvm_fatal("IRQ_WAIT_LEAK","Timed-out receiver consumed a later notification")
        if(!unrelated_done) `uvm_fatal("IRQ_WAIT_SCOPE","Timeout killed an unrelated caller-owned thread")
    endtask
endclass
class pdma_irq_wait_reentrant_test extends pdma_tb_test;
    `uvm_component_utils(pdma_irq_wait_reentrant_test)
    function new(string name,uvm_component parent); super.new(name,parent); endfunction
    virtual task check_tb();
        pdma_channel first=make_channel("first"), second=make_channel("second");
        pdma_result first_event=new, second_event=new, first_result, second_result;
        fork
            first.wait_interrupt(first_result);
            second.wait_interrupt(second_result);
            begin
                repeat(2) @(posedge control.core_clk); first.interrupts.put(first_event);
                repeat(8) @(posedge control.core_clk); second.interrupts.put(second_event);
            end
        join
        if(first_result!=first_event || second_result!=second_event)
            `uvm_fatal("IRQ_WAIT_REENTRANT","One wait interfered with another invocation")
    endtask
endclass
