class pdma_configuration extends uvm_object;
    // Fixed DUT profile and AXI burst boundary, not randomized platform settings.
    localparam int unsigned REGISTER_DATA_BITS=32, MEMORY_DATA_BITS=64;
    localparam int unsigned REGISTER_WORD_BYTES=REGISTER_DATA_BITS/8;
    localparam int unsigned MEMORY_BEAT_BYTES=MEMORY_DATA_BITS/8, AXI_BOUNDARY_BYTES=4096;
    int channel_count=`PDMA_CHANNEL_COUNT;
    int unsigned command_capacity=4, event_capacity=2;
    bit[64:0] command_ring_address=`JVS_MEM_NULL, event_ring_address=`JVS_MEM_NULL;
    longint unsigned heap_base='h10000, heap_size='h100000;
    int unsigned poll_limit=4000, irq_timeout_cycles=4000;
    bit[31:0] ring_context=0;
    int irq_vectors[];
    int device_irq_vector=16;
    int delay_max=7;
    int write_address_delay=-1, write_data_ready_delay=-1;
    bit memory_default_ready;
    int memory_outstanding=1;
    bit fault_enabled, fault_write, fault_page;
    bit [63:0] fault_address;
    int injected=0;
    int w_first_faults=0;
    bit hold_write;
    svt_axi_transaction held[$];
    function new(string name="configuration");
        super.new(name);
        irq_vectors=new[channel_count];
        foreach(irq_vectors[i]) irq_vectors[i]=17+i;
    endfunction
    function svt_axi_system_configuration system_config(bit software_port);
        svt_axi_system_configuration c=new;
        svt_axi_port_configuration p;
        c.num_masters=software_port ? 1 : 0;
        c.num_slaves=software_port ? 0 : 1;
        c.create_sub_cfgs(c.num_masters,c.num_slaves);
        p=software_port ? c.master_cfg[0] : c.slave_cfg[0];
        p.is_active=1;
        p.axi_interface_type=software_port ? svt_axi_port_configuration::AXI4_LITE : svt_axi_port_configuration::AXI4;
        p.addr_width=software_port ? REGISTER_DATA_BITS : MEMORY_DATA_BITS;
        p.data_width=software_port ? REGISTER_DATA_BITS : MEMORY_DATA_BITS;
        p.id_width=1;
        p.wysiwyg_enable=1;
        p.num_outstanding_xact=software_port ? 1 : memory_outstanding;
        p.transaction_coverage_enable=1;
        p.per_clock_cycle_signal_checks_enable=1;
        if(software_port) begin
            p.default_bready=0; p.default_rready=0;
        end else begin
            p.addr_user_width=35; p.data_user_width=2; p.resp_user_width=2;
            p.awuser_enable=1; p.aruser_enable=1; p.ruser_enable=1; p.buser_enable=1;
            p.default_awready=memory_default_ready;
            p.default_arready=memory_default_ready; p.default_wready=memory_default_ready;
            p.read_data_reordering_depth=1; p.write_resp_reordering_depth=1;
            c.set_addr_range(0,0,64'hffff_ffff_ffff_ffff);
        end
        return c;
    endfunction
endclass
