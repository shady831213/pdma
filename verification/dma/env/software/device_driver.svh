// Global registers and global reset policy belong to the device driver.
// Channels retain ring allocation; task flows retain task-buffer ownership.
class pdma_device extends pdma_register_access;
    DmaControl registers;
    pdma_channel channels[];
    int unsigned fatal_irqs_serviced;
    function new(pdma_configuration cfg,DmaRegisters root,pdma_memory memory,
            jvs_memory_allocator allocator);
        super.new("device");
        this.cfg=cfg;
        registers=root.admin_block.control_block;
        channels=new[cfg.channel_count];
        foreach(channels[i]) begin
            channels[i]=new($sformatf("channel_%0d",i));
            channels[i].bind_registers(root,i);
            channels[i].cfg=cfg;
            channels[i].allocator=allocator;
            channels[i].memory=memory;
        end
    endfunction
    task service_irq(output pdma_result result);
        uvm_reg_data_t fatal_code;
        read_reg(registers.fatal_reg,fatal_code);
        if(fatal_code==0) `uvm_fatal("DEVICE_IRQ_CAUSE","Device notification has no fatal cause")
        result=new; result.outcome=TASK_FATAL; result.code=fatal_code;
        write_reg(registers.irq_enable_reg,0);
        fatal_irqs_serviced++;
    endtask
    task enable_notifications();
        write_reg(registers.irq_enable_reg,1<<registers.irq_enable_reg.enabled_field.get_lsb_pos());
    endtask
    task request_reset();
        write_reg(registers.reset_reg,1<<registers.reset_reg.request_field.get_lsb_pos());
    endtask
    task finish_reset();
        await_reg(registers.reset_reg,1<<registers.reset_reg.request_field.get_lsb_pos(),0);
        foreach(channels[i]) channels[i].invalidate_after_reset();
    endtask
endclass
