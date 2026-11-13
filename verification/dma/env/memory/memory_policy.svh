// Address-dependent memory policy follows actual VIP address observations. In
// particular a W-first notification has no usable AW address yet. The VIP owns
// transaction assembly, W readiness and the eventual backing-memory update.
class pdma_memory_policy extends svt_axi_port_monitor_callback;
    pdma_configuration behavior;
    pdma_memory memory;
    function new(string name="memory_policy"); super.new(name); endfunction
    function void apply_access(svt_axi_transaction item);
        bit writing=item.xact_type==svt_axi_transaction::WRITE;
        bit inject;
        memory.assert_access(item.addr,item.burst_length*(1<<item.burst_size),item.addr_user[31:0],writing,item.addr_user[34:32]);
        inject=behavior.fault_enabled && item.addr==behavior.fault_address && writing==behavior.fault_write;
        if(inject) behavior.injected++;
        if(inject && writing && item.data_status==svt_sequence_item::ACCEPT) behavior.w_first_faults++;
        if(writing) begin
            item.bresp=inject ? svt_axi_transaction::SLVERR : svt_axi_transaction::OKAY;
            item.resp_user=inject && behavior.fault_page ? 1 : 0;
        end else begin
            foreach(item.rresp[i]) item.rresp[i]=inject ? svt_axi_transaction::SLVERR : svt_axi_transaction::OKAY;
            foreach(item.data_user[i]) item.data_user[i]=inject && behavior.fault_page ? 1 : 0;
        end
    endfunction
    virtual function void write_address_phase_ended(svt_axi_port_monitor monitor_,svt_axi_transaction item);
        apply_access(item);
    endfunction
endclass
