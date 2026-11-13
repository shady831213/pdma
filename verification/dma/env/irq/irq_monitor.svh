class pdma_notification extends uvm_component;
    `uvm_component_utils(pdma_notification)
    virtual pdma_control_if control;
    jvs_irq_vir_sequencer sequencer;
    pdma_channel channels[];
    int device_vector;
    function new(string name,uvm_component parent); super.new(name,parent); endfunction
    virtual task run_phase(uvm_phase phase);
        forever begin
            @(control.observer);
            if(control.observer.core_rst_n && control.observer.fatal_irq && control.observer.fatal_irq_ready) begin
                jvs_irq_trans tr=new("device_fatal_notification");
                tr.irq_vector=device_vector;
                sequencer.write(tr);
            end
            if(control.observer.core_rst_n) foreach(channels[i])
                if(control.observer.irq[i] && control.observer.irq_ready[i]) begin
                    jvs_irq_trans tr=new("accepted_notification");
                    tr.irq_vector=channels[i].vector;
                    sequencer.write(tr);
                end
        end
    endtask
endclass
