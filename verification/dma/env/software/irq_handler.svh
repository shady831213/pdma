class pdma_irq_handler extends jvs_irq_handler;
    pdma_channel channel;
    function new(string name="handler"); super.new(name); endfunction
    virtual task do_handle(jvs_irq_trans irq_tr);
        pdma_result result;
        if(irq_tr.irq_vector!=channel.vector) `uvm_fatal("TASK_IDENTITY","IRQ routed to the wrong channel handler")
        channel.interrupt_snapshot(result);
        channel.delivered++;
        channel.interrupts.put(result);
        irq_tr.handle_state=jvs_irq_trans::FINISH;
    endtask
    virtual function jvs_irq_result_trans gen_result(jvs_irq_trans irq_tr); return null; endfunction
endclass
