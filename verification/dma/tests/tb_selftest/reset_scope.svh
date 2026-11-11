class pdma_reset_scope_probe extends pdma_clocks;
    `uvm_component_utils(pdma_reset_scope_probe)
    bit scope_verified;
    function new(string name,uvm_component parent); super.new(name,parent); endfunction
    virtual task reset_phase(uvm_phase phase);
        process unrelated;
        phase.raise_objection(this);
        fork begin
            unrelated=process::self();
            forever @(posedge vif[0].clock);
        end join_none
        wait(unrelated!=null);
        super.reset_phase(phase);
        if(unrelated.status()==process::KILLED)
            `uvm_fatal("RESET_WAIT_SCOPE","Reset killed an unrelated caller-owned thread")
        unrelated.kill();
        scope_verified=1;
        phase.drop_objection(this);
    endtask
endclass
class pdma_reset_wait_isolation_test extends pdma_tb_test;
    `uvm_component_utils(pdma_reset_wait_isolation_test)
    function new(string name,uvm_component parent); super.new(name,parent); endfunction
    virtual function void build_phase(uvm_phase phase);
        pdma_clocks::type_id::set_type_override(pdma_reset_scope_probe::get_type());
        super.build_phase(phase);
    endfunction
    virtual task check_tb();
        pdma_reset_scope_probe probe;
        if(!$cast(probe,clocks) || !probe.scope_verified)
            `uvm_fatal("RESET_WAIT_SCOPE","Reset scope check did not execute")
    endtask
endclass
