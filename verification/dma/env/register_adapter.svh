// AXI4-Lite has a fixed bus beat, even when RAL accesses a 32-bit register on
// a 64-bit port. Preserve the register address; place data/strobes in its lane.
class pdma_register_adapter extends svt_axi_reg_adapter;
    `uvm_object_utils(pdma_register_adapter)
    function new(string name="register_adapter"); super.new(name); endfunction
    virtual function uvm_sequence_item reg2bus(const ref uvm_reg_bus_op rw);
        uvm_reg_bus_op bus_op=rw;
        uvm_sequence_item item;
        svt_axi_master_transaction transaction;
        int unsigned bytes=p_cfg.data_width/8;
        int unsigned lane=rw.addr%bytes;
        if(rw.n_bits+lane*8>p_cfg.data_width)
            `uvm_fatal("REGISTER_LANES","RAL transfer crosses an AXI4-Lite beat")
        bus_op.n_bits=p_cfg.data_width;
        bus_op.data=rw.data<<(lane*8);
        item=super.reg2bus(bus_op);
        if(!$cast(transaction,item)) `uvm_fatal("REGISTER_ADAPTER","Unexpected bus item type")
        if(rw.kind==UVM_WRITE) begin
            transaction.wstrb[0]='0;
            for(int i=0;i<(rw.n_bits+7)/8;i++)
                transaction.wstrb[0][lane+i]=rw.byte_en[i];
        end
        return item;
    endfunction
    virtual function void bus2reg(uvm_sequence_item bus_item, ref uvm_reg_bus_op rw);
        super.bus2reg(bus_item,rw);
        rw.data >>= (rw.addr%(p_cfg.data_width/8))*8;
    endfunction
endclass
