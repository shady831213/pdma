class pdma_mmio extends svt_axi_master_base_sequence;
    `uvm_object_utils(pdma_mmio)
    bit write_access;
    bit[31:0] address, value;
    svt_axi_master_transaction result;
    function new(string name="mmio"); super.new(name); endfunction
    virtual task body();
        super.body();
        req=svt_axi_master_transaction::type_id::create("request");
        start_item(req);
        if(!req.randomize() with {
            xact_type==(local::write_access ? svt_axi_transaction::WRITE : svt_axi_transaction::READ);
            addr==local::address; burst_length==1; burst_size==2;
            burst_type==svt_axi_transaction::INCR; id==0;
            atomic_type==0; prot_type==svt_axi_transaction::DATA_SECURE_NORMAL;
            cache_type==0; addr_valid_delay inside {[0:8]};
            foreach(wvalid_delay[i]) wvalid_delay[i] inside {[0:8]};
            foreach(rready_delay[i]) rready_delay[i] inside {[0:8]};
            bready_delay inside {[0:8]};
        }) `uvm_fatal("MMIO_RANDOMIZE","Cannot generate MMIO")
        req.data[0]=value; req.wstrb[0]=4'hf;
        finish_item(req);
        get_response(result);
        if(write_access ? result.bresp!=svt_axi_transaction::OKAY : result.rresp[0]!=svt_axi_transaction::OKAY)
            `uvm_fatal("MMIO_RESPONSE",$sformatf("Rejected register access at %h",address))
        if(!write_access) value=result.data[0];
    endtask
endclass
