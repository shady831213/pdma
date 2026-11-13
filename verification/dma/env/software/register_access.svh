// Software frontdoor access and bounded polling shared by device/channel owners.
virtual class pdma_register_access extends uvm_object;
    pdma_configuration cfg;
    function new(string name); super.new(name); endfunction
    task write_reg(uvm_reg register_,uvm_reg_data_t value);
        uvm_status_e status;
        register_.write(status,value,UVM_FRONTDOOR);
        if(status!=UVM_IS_OK)
            `uvm_fatal("MMIO_RESPONSE",$sformatf("Rejected register write: %s",register_.get_full_name()))
    endtask
    task read_reg(uvm_reg register_,output uvm_reg_data_t value);
        uvm_status_e status;
        register_.read(status,value,UVM_FRONTDOOR);
        if(status!=UVM_IS_OK)
            `uvm_fatal("MMIO_RESPONSE",$sformatf("Rejected register read: %s",register_.get_full_name()))
    endtask
    task await_reg(uvm_reg register_,uvm_reg_data_t mask,uvm_reg_data_t expected);
        uvm_reg_data_t value;
        for(int n=0;n<cfg.poll_limit;n++) begin
            read_reg(register_,value);
            if((value & mask)==expected) return;
        end
        `uvm_fatal("TASK_TIMEOUT",$sformatf("owner=%s register=%s expected=%h observed=%h",get_name(),register_.get_full_name(),expected,value))
    endtask
endclass
