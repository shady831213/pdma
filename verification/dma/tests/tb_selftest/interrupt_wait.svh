// Supply the MMIO observation at the ISR boundary; use the production callback,
// JarvisUK result FIFO and scenario consumer for delivery.
class pdma_test_irq_source;
    pdma_result observation;
    task service_irq(output pdma_result result); result=observation; endtask
endclass
class pdma_irq_wait_flow extends pdma_task_flow;
    local pdma_test_irq_source source=new;
    function new(string name);
        pdma_irq_env#(pdma_test_irq_source)::callback handler;
        super.new(name);
        handler=new(source); irq_handler=handler;
        channel=new(name); channel.cfg=new; channel.cfg.irq_timeout_ns=1000;
    endfunction
    task deliver(pdma_result observation);
        jvs_irq_trans irq=new;
        source.observation=observation;
        irq_handler.handle(irq);
    endtask
endclass

class pdma_irq_wait_isolation_test extends pdma_tb_test;
    `uvm_component_utils(pdma_irq_wait_isolation_test)
    function new(string name,uvm_component parent); super.new(name,parent); endfunction
    virtual task check_tb();
        pdma_irq_wait_flow flow_=new("channel");
        pdma_result expected=new, actual;
        bit wait_returned=0, unrelated_done=0;
        fork
            begin wait(wait_returned); unrelated_done=1; end
            begin repeat(2) @(posedge control.core_clk); flow_.deliver(expected); end
        join_none
        flow_.wait_irq_result(actual);
        wait_returned=1;
        if(actual!=expected) `uvm_fatal("IRQ_IDENTITY","Wait returned another notification")
        @(posedge control.core_clk);
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
        pdma_irq_wait_flow flow_=new("channel");
        pdma_expected_irq_timeout catcher=new;
        pdma_result late=new, actual, remaining;
        bit wait_returned=0, unrelated_done=0;
        flow_.channel.cfg.irq_timeout_ns=100;
        uvm_report_cb::add(null,catcher);
        fork begin wait(wait_returned); unrelated_done=1; end join_none
        flow_.wait_irq_result(actual);
        wait_returned=1;
        uvm_report_cb::delete(null,catcher);
        if(catcher.caught!=1 || actual!=null) `uvm_fatal("IRQ_TIMEOUT","Expected exactly one timeout without a result")
        flow_.deliver(late);
        repeat(12) @(posedge control.core_clk);
        flow_.wait_irq_result(remaining);
        if(remaining!=late)
            `uvm_fatal("IRQ_WAIT_LEAK","Timed-out receiver consumed a later notification")
        if(!unrelated_done) `uvm_fatal("IRQ_WAIT_SCOPE","Timeout killed an unrelated caller-owned thread")
    endtask
endclass
class pdma_irq_wait_reentrant_test extends pdma_tb_test;
    `uvm_component_utils(pdma_irq_wait_reentrant_test)
    function new(string name,uvm_component parent); super.new(name,parent); endfunction
    virtual task check_tb();
        pdma_irq_wait_flow first=new("first"), second=new("second");
        pdma_result first_event=new, second_event=new, first_result, second_result;
        fork
            first.wait_irq_result(first_result);
            second.wait_irq_result(second_result);
            begin
                repeat(2) @(posedge control.core_clk); first.deliver(first_event);
                wait(first_result!=null); second.deliver(second_event);
            end
        join
        if(first_result!=first_event || second_result!=second_event)
            `uvm_fatal("IRQ_WAIT_REENTRANT","One wait interfered with another invocation")
    endtask
endclass
