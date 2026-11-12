`timescale 1ns/1ps
// Independent RTL oracle: free-running clocks and AXI handshakes, no DUT internals.
module MinimalCopyTest;
    import pdma_contract_records::*;
    localparam int WORD_BYTES=8, BURST_BYTES=64, COPY_BYTES=128, RING_CAPACITY=4;
    localparam int RING_BYTES=RING_CAPACITY*COMMAND_SIZE;
    localparam int SOURCE_ADDRESS='h200, DEST_ADDRESS='h300, FENCE_ADDRESS='h380;
    localparam int RING_CONTEXT=7, SOURCE_CONTEXT=11, DEST_CONTEXT=13;
    logic reg_clk = 0, core_clk = 0, bus_clk = 0;
    always #5 reg_clk = ~reg_clk;
    always #7 core_clk = ~core_clk;
    always #4 bus_clk = ~bus_clk;
    logic reg_rst_n = 0, core_rst_n = 0, bus_rst_n = 0;
    __std___axi4_lite_if____std___axi4_lite_pkg__32__4__1 cfg();
    __std___axi4_if____std___axi4_pkg__64__8__1__32__1__2__32__2 memory();
    logic [5:0] awatop;
    logic irq, fatal;
    pdma_rtl_DmaCopy dut(.*,.irq_ready(1'b1));
    logic [63:0] words[0:127];
    logic reading = 0, aw_seen = 0, w_seen = 0, b_pending = 0;
    logic hold_aw = 0, hold_b = 0;
    logic [63:0] read_addr, write_addr;
    int read_beat, read_len, write_beat = 0, write_len;
    logic [63:0] write_data[0:BURST_BYTES/WORD_BYTES-1];
    logic [7:0] write_strobe[0:BURST_BYTES/WORD_BYTES-1];
    int completed_writes = 0;
    bit quiet=0;
    assign memory.arready = !reading;
    assign memory.rvalid = reading;
    assign memory.rdata = words[read_addr >> 3];
    assign memory.rlast = read_beat == read_len;
    assign memory.rid = 0;
    assign memory.rresp = '0;
    assign memory.ruser = 0;
    assign memory.awready = !hold_aw && !aw_seen && !b_pending;
    assign memory.wready = !w_seen && !b_pending;
    assign memory.bvalid = b_pending && !hold_b;
    assign memory.bresp = '0;
    assign memory.bid = 0;
    assign memory.buser = 0;
    always @(posedge bus_clk) if (bus_rst_n) begin
        if(quiet && (memory.arvalid || memory.awvalid || memory.wvalid ||
                     reading || aw_seen || w_seen || b_pending))
            $fatal(1,"Old memory access survived reset completion");
        if (memory.arvalid && memory.arready) begin
            if (memory.arlen >= BURST_BYTES/WORD_BYTES || memory.arsize != $clog2(WORD_BYTES) || memory.arid != 0)
                $fatal(1, "Unexpected read geometry");
            if (memory.aruser != (memory.araddr < RING_BYTES ? RING_CONTEXT : SOURCE_CONTEXT))
                $fatal(1, "Read context mismatch");
            if(memory.araddr<RING_BYTES) begin
                if(memory.araddr+(memory.arlen+1)*WORD_BYTES>RING_BYTES) $fatal(1,"Descriptor read overflow");
            end else if(memory.araddr<SOURCE_ADDRESS ||
                        memory.araddr+(memory.arlen+1)*WORD_BYTES>SOURCE_ADDRESS+COPY_BYTES)
                $fatal(1,"Payload read outside scenario range");
            read_addr <= memory.araddr;
            read_len <= memory.arlen;
            read_beat <= 0;
            reading <= 1;
        end
        if (memory.rvalid && memory.rready) begin
            if (memory.rlast) reading <= 0;
            else begin read_beat <= read_beat + 1; read_addr <= read_addr + 8; end
        end
        if (memory.awvalid && memory.awready) begin
            if (memory.awuser != DEST_CONTEXT || memory.awlen >= BURST_BYTES/WORD_BYTES || memory.awsize != $clog2(WORD_BYTES))
                $fatal(1, "Write identity/geometry mismatch");
            if(memory.awaddr!=FENCE_ADDRESS && (memory.awaddr<DEST_ADDRESS ||
               memory.awaddr+(memory.awlen+1)*WORD_BYTES>DEST_ADDRESS+COPY_BYTES))
                $fatal(1,"Payload write outside scenario range");
            if(memory.awaddr==FENCE_ADDRESS && memory.awlen!=0) $fatal(1,"Fence write overflow");
            aw_seen <= 1; write_addr <= memory.awaddr; write_len <= memory.awlen;
        end
        if (memory.wvalid && memory.wready) begin
            if(write_beat>=BURST_BYTES/WORD_BYTES) $fatal(1,"Write burst exceeded responder capacity");
            write_data[write_beat] <= memory.wdata;
            write_strobe[write_beat] <= memory.wstrb;
            if (memory.wlast) w_seen <= 1;
            else write_beat <= write_beat + 1;
        end
        if (aw_seen && w_seen && !b_pending) begin
            if(write_beat!==write_len) $fatal(1,"Write address/data length mismatch");
            for (int i=0; i<=write_len; i++)
                for (int b=0; b<8; b++)
                    if (write_strobe[i][b]) words[(write_addr >> 3)+i][b*8+:8] <= write_data[i][b*8+:8];
            b_pending <= 1;
        end
        if (memory.bvalid && memory.bready) begin
            b_pending <= 0; aw_seen <= 0; w_seen <= 0; write_beat <= 0;
            completed_writes <= completed_writes + 1;
        end
    end
    task automatic write_reg(input logic[31:0] address, data);
        @(negedge reg_clk);
        cfg.awaddr = address; cfg.wdata = data;
        cfg.awvalid = 1; cfg.wvalid = 1;
        fork
            begin
                do @(posedge reg_clk); while (!cfg.awready);
                @(negedge reg_clk); cfg.awvalid = 0;
            end
            begin
                do @(posedge reg_clk); while (!cfg.wready);
                @(negedge reg_clk); cfg.wvalid = 0;
            end
        join
        cfg.bready = 1;
        do @(posedge reg_clk); while (!cfg.bvalid);
        if (cfg.bresp != 0) $fatal(1, "MMIO write rejected at %h", address);
        @(negedge reg_clk); cfg.bready = 0;
    endtask
    task automatic read_reg(input logic[31:0] address, output logic[31:0] data);
        @(negedge reg_clk); cfg.araddr = address; cfg.arvalid = 1;
        do @(posedge reg_clk); while (!cfg.arready);
        @(negedge reg_clk); cfg.arvalid = 0; cfg.rready = 1;
        do @(posedge reg_clk); while (!cfg.rvalid);
        if (cfg.rresp != 0) $fatal(1, "MMIO read rejected");
        data = cfg.rdata;
        @(negedge reg_clk); cfg.rready = 0;
    endtask
    localparam int CHANNEL = pdma_rtl_DmaRegistersLayout::channels_offset(1);
    localparam int RESET = pdma_rtl_DmaRegistersLayout::admin_offset(1)
        + pdma_rtl_DmaAdminLayout::channels_offset(1);
    function automatic Command_t copy_record(input logic[63:0] id);
        CopyCommand_t value='0;
        CommandHeader_t header='0;
        LinearEndpoint_t source='0, destination='0;
        CommandHeader_set_opcode(header,COMMANDHEADER_OPCODE_COPY);
        LinearEndpoint_set_address(source,SOURCE_ADDRESS);
        LinearEndpoint_set_address(destination,DEST_ADDRESS);
        CopyCommand_set_header(value,header); CopyCommand_set_id(value,id);
        CopyCommand_set_copy_length(value,COPY_BYTES);
        CopyCommand_set_source_context(value,SOURCE_CONTEXT);
        CopyCommand_set_destination_context(value,DEST_CONTEXT);
        CopyCommand_set_source(value,source); CopyCommand_set_destination(value,destination);
        return Command_t'(value);
    endfunction
    function automatic Command_t fence_record(input logic[63:0] id, marker);
        FenceCommand_t value='0;
        CommandHeader_t header='0;
        WordTarget_t target='0;
        WordWrite_t word_value='0;
        CommandHeader_set_opcode(header,COMMANDHEADER_OPCODE_FENCE);
        WordTarget_set_address(target,FENCE_ADDRESS); WordTarget_set_context(target,DEST_CONTEXT);
        WordTarget_set_width(target,WORDTARGET_WIDTH_BITS64);
        WordWrite_set_target(word_value,target); WordWrite_set_value(word_value,marker);
        FenceCommand_set_header(value,header); FenceCommand_set_id(value,id);
        FenceCommand_set_word(value,word_value);
        return Command_t'(value);
    endfunction
    task automatic install(input int slot, input Command_t record_value);
        for(int i=0;i<COMMAND_SIZE/WORD_BYTES;i++)
            words[slot*COMMAND_SIZE/WORD_BYTES+i]=record_value[i*64+:64];
    endtask
    task automatic await_fence(input logic[63:0] marker);
        do @(negedge bus_clk); while(words[FENCE_ADDRESS/WORD_BYTES]!==marker);
        for(int i=0;i<COPY_BYTES/WORD_BYTES;i++)
            if(words[DEST_ADDRESS/WORD_BYTES+i]!==words[SOURCE_ADDRESS/WORD_BYTES+i])
                $fatal(1,"Fence preceded correct Copy data");
    endtask
    task automatic configure;
        write_reg(CHANNEL + pdma_rtl_ChannelBlockLayout::COMMAND_CAPACITY_OFFSET, RING_CAPACITY);
        write_reg(CHANNEL + pdma_rtl_ChannelBlockLayout::CONTROL_CONFIG_RING_CONTEXT_OFFSET, RING_CONTEXT);
        write_reg(CHANNEL + pdma_rtl_ChannelBlockLayout::COMMAND_PRODUCER_OFFSET, 2);
        write_reg(CHANNEL + pdma_rtl_ChannelBlockLayout::CONTROL_CONFIG_ENABLE_OFFSET, 1);
    endtask
    logic [31:0] value;
    int before_reset, after_reset;
    initial begin
        cfg.awvalid=0; cfg.wvalid=0; cfg.bready=0; cfg.arvalid=0; cfg.rready=0;
        cfg.awaddr=0; cfg.awprot='0; cfg.awid=0; cfg.wdata=0; cfg.wstrb='1;
        cfg.araddr=0; cfg.arprot='0; cfg.arid=0;
        for (int i=0; i<128; i++) words[i] = 64'hdeadbeefdeadbeef;
        install(0,copy_record('h100)); install(1,fence_record('h101,1));
        install(2,copy_record('h200)); install(3,fence_record('h201,2));
        words[FENCE_ADDRESS/WORD_BYTES]=0;
        for (int i=0; i<COPY_BYTES/WORD_BYTES; i++) words[SOURCE_ADDRESS/WORD_BYTES+i]=64'h1020304050607000+i;
        #100;
        fork
            begin @(negedge reg_clk); reg_rst_n=1; end
            begin @(negedge core_clk); core_rst_n=1; end
            begin @(negedge bus_clk); bus_rst_n=1; end
        join
        repeat (20) @(posedge reg_clk);
        configure();
        await_fence(1);
        // Injection setup waits for the responder to finish the prior Fence's B.
        // Software completion itself was observed through Fence storage above.
        wait(!b_pending && !aw_seen && !w_seen);
        hold_aw=1; hold_b=1;
        words[FENCE_ADDRESS/WORD_BYTES]=0;
        write_reg(CHANNEL + pdma_rtl_ChannelBlockLayout::COMMAND_PRODUCER_OFFSET, 4);
        wait (memory.awvalid && w_seen);
        before_reset=completed_writes;
        write_reg(RESET, 1);
        read_reg(RESET, value);
        if (value != 1 || !memory.awvalid) $fatal(1, "Reset retracted AW");
        @(negedge bus_clk); hold_aw=0;
        wait (b_pending);
        read_reg(RESET, value);
        if (value != 1) $fatal(1, "Reset completed before B");
        @(negedge bus_clk); hold_b=0;
        do read_reg(RESET, value); while (value != 0);
        after_reset=completed_writes;
        // A full-width transaction carries at least one word. Do not assume
        // that the implementation always chooses the maximum burst length.
        if(after_reset<=before_reset || after_reset>before_reset+COPY_BYTES/WORD_BYTES)
            $fatal(1,"Reset lost accepted work or executed beyond the active Copy");
        if(words[FENCE_ADDRESS/WORD_BYTES]!==0) $fatal(1,"Successor Fence passed reset");
        quiet=1;
        repeat(20) @(posedge bus_clk);
        @(negedge bus_clk);
        if(completed_writes!=after_reset) $fatal(1,"Late write after reset completion");
        quiet=0;
        for(int i=0;i<COPY_BYTES/WORD_BYTES;i++) words[SOURCE_ADDRESS/WORD_BYTES+i]=64'h9080706050403000+i;
        install(0,copy_record('h300)); install(1,fence_record('h301,3));
        configure(); await_fence(3);
        if (fatal || irq || awatop != 0) $fatal(1, "Unexpected fatal/IRQ/atomic");
        $display("PDMA_MINIMAL_SV_PASS");
        $finish;
    end
    initial begin #200000; $fatal(1, "Global test timeout"); end
endmodule
