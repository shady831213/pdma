// RAL emits whole register accesses. Drive one AXI4-Lite beat when the
// byte enables or the unaligned address are part of the stimulus.
class pdma_register_beat extends svt_axi_master_base_sequence;
    bit writing;
    uvm_reg_addr_t address;
    bit [63:0] data;
    bit [7:0] strobes;
    svt_axi_master_transaction result;
    function new(string name="register_beat"); super.new(name); endfunction
    virtual task body();
        super.body();
        req=svt_axi_master_transaction::type_id::create("request");
        start_item(req);
        if(!req.randomize() with {
            xact_type==(local::writing ? svt_axi_transaction::WRITE : svt_axi_transaction::READ);
            addr==local::address;
            burst_length==1;
            burst_size==$clog2(pdma_configuration::REGISTER_WORD_BYTES);
        }) `uvm_fatal("MMIO_STIMULUS","Cannot generate register access")
        if(writing) begin
            req.data[0]=data;
            req.wstrb[0]=strobes;
        end
        finish_item(req);
        get_response(result);
    endtask
endclass

class pdma_register_strobes_test extends pdma_base_test;
    `uvm_component_utils(pdma_register_strobes_test)
    function new(string name,uvm_component parent); super.new(name,parent); endfunction
    local task access(bit writing,uvm_reg_addr_t address,bit [63:0] data,bit [7:0] strobes,
            bit [1:0] expected);
        pdma_register_beat beat=new;
        bit [1:0] response;
        beat.writing=writing; beat.address=address; beat.data=data; beat.strobes=strobes;
        beat.start(env.host_bus.master[0].sequencer);
        response=writing ? beat.result.bresp : beat.result.rresp[0];
        if(response!==expected || (!writing && beat.result.data[0]!==0))
            `uvm_fatal("MMIO_RESPONSE",$sformatf("write=%b address=%h strobe=%h expected=%0d response=%0d data=%h",
                writing,address,strobes,expected,response,beat.result.data[0]))
    endtask
    virtual task run_scenario();
        localparam int BUS_BYTES=pdma_configuration::REGISTER_WORD_BYTES;
        pdma_task_flow flow_=env.device.task_flow(platform.channel_count-1);
        pdma_channel channel=flow_.channel;
        uvm_reg base=channel.registers.command_block.base_reg;
        uvm_reg_addr_t address=base.get_address();
        uvm_reg_data_t original={$urandom,$urandom},observed;
        channel.reset();
        channel.write_reg(base,original);
        access(1,address,~original,0,svt_axi_transaction::OKAY);
        // Legal AXI strobes that do not cover one complete PDMA word.
        for(int lane=0;lane<BUS_BYTES;lane++)
            access(1,address,~original,1<<lane,svt_axi_transaction::SLVERR);
        access(0,address+1,0,0,svt_axi_transaction::SLVERR);
        channel.read_reg(base,observed);
        if(observed!==original) `uvm_fatal("MMIO_STROBE","Empty or rejected write changed the command base")
        // Replace each half with the other half still observable through RAL.
        for(int half=0;half<2;half++) begin
            int lane=(half*4)%BUS_BYTES;
            bit [31:0] word_=~original[32*half+:32];
            access(1,address+4*half,64'(word_)<<(8*lane),8'h0f<<lane,svt_axi_transaction::OKAY);
            original[32*half+:32]=word_;
            channel.read_reg(base,observed);
            if(observed!==original) `uvm_fatal("MMIO_STROBE","Word write corrupted its neighbor or missed its lane")
        end
        flow_.open(settings()); flow_.submit(); flow_.expect_fence(); flow_.close(); cases++;
    endtask
endclass
