// Global registers and global reset policy belong to the device driver.
// Channels retain ring allocation; task flows retain task-buffer ownership.
class pdma_device extends pdma_register_access;
    DmaControl registers;
    pdma_channel channels[];
    function new(string name="device"); super.new(name); endfunction
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
