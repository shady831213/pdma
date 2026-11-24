// A JarvisUK service connects one physical IRQ to its software owner.
class pdma_irq_env #(type DRIVER) extends jvs_irq_env #(1);
    `uvm_component_param_utils(pdma_irq_env #(DRIVER))
    class callback extends jvs_irq_handler;
        local DRIVER owner;
        function new(DRIVER owner); super.new("handler"); this.owner=owner; endfunction
        virtual task do_handle(jvs_irq_trans irq_tr);
            pdma_result observation;
            owner.service_irq(observation);
            irq_tr.attr=observation;
            irq_tr.handle_state=jvs_irq_trans::FINISH;
        endtask
        virtual function jvs_irq_result_trans gen_result(jvs_irq_trans irq_tr);
            jvs_irq_result_trans result;
            result=new("irq_result");
            result.irq_vector=irq_tr.irq_vector;
            result.attr=irq_tr.attr;
            return result;
        endfunction
    endclass
    callback handler;
    function new(string name,uvm_component parent); super.new(name,parent); endfunction
    virtual task configure_phase(uvm_phase phase);
        phase.raise_objection(this);
        register_irq(0,handler);
        phase.drop_objection(this);
    endtask
endclass
