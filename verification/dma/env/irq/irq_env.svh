// One physical notification output has its own JarvisUK FIFO and ISR service.
class pdma_irq_env extends jvs_irq_env;
    `uvm_component_utils(pdma_irq_env)
    int vector;
    jvs_irq_handler handler;
    function new(string name,uvm_component parent); super.new(name,parent); endfunction
    virtual task configure_phase(uvm_phase phase);
        phase.raise_objection(this);
        register_irq(vector,handler);
        phase.drop_objection(this);
    endtask
endclass
