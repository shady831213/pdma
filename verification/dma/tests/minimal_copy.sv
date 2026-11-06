`timescale 1ns/1ps
// Independent RTL oracle: free-running clocks and AXI handshakes, no DUT internals.
module MinimalCopyTest;
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
    logic [63:0] write_data[0:7];
    logic [7:0] write_strobe[0:7];
    int completed_writes = 0;
    int bus_cycles = 0, first_write_cycle = 0;
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
        bus_cycles <= bus_cycles + 1;
        if (memory.arvalid && memory.arready) begin
            if (memory.arlen > 7 || memory.arsize != 3 || memory.arid != 0)
                $fatal(1, "Unexpected read geometry");
            if (memory.aruser != (memory.araddr < 192 ? 7 : 11))
                $fatal(1, "Read context mismatch");
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
            if (memory.awuser != 13 || memory.awlen > 7 || memory.awsize != 3)
                $fatal(1, "Write identity/geometry mismatch");
            aw_seen <= 1; write_addr <= memory.awaddr; write_len <= memory.awlen;
        end
        if (memory.wvalid && memory.wready) begin
            write_data[write_beat] <= memory.wdata;
            write_strobe[write_beat] <= memory.wstrb;
            if (memory.wlast) w_seen <= 1;
            else write_beat <= write_beat + 1;
        end
        if (aw_seen && w_seen && !b_pending) begin
            for (int i=0; i<=write_len; i++)
                for (int b=0; b<8; b++)
                    if (write_strobe[i][b]) words[(write_addr >> 3)+i][b*8+:8] <= write_data[i][b*8+:8];
            b_pending <= 1;
        end
        if (memory.bvalid && memory.bready) begin
            if (completed_writes == 0) first_write_cycle <= bus_cycles;
            if (completed_writes == 1)
                $display("COPY_SV_BASELINE bytes=64 bus_cycles=%0d core_period_ns=14 bus_period_ns=8", bus_cycles-first_write_cycle);
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
    task automatic configure;
        write_reg(CHANNEL + pdma_rtl_ChannelBlockLayout::COMMAND_CAPACITY_OFFSET, 2);
        write_reg(CHANNEL + pdma_rtl_ChannelBlockLayout::CONTROL_CONFIG_RING_CONTEXT_OFFSET, 7);
        write_reg(CHANNEL + pdma_rtl_ChannelBlockLayout::COMMAND_PRODUCER_OFFSET, 1);
        write_reg(CHANNEL + pdma_rtl_ChannelBlockLayout::CONTROL_CONFIG_ENABLE_OFFSET, 1);
    endtask
    logic [31:0] value;
    initial begin
        cfg.awvalid=0; cfg.wvalid=0; cfg.bready=0; cfg.arvalid=0; cfg.rready=0;
        cfg.awaddr=0; cfg.awprot='0; cfg.awid=0; cfg.wdata=0; cfg.wstrb='1;
        cfg.araddr=0; cfg.arprot='0; cfg.arid=0;
        for (int i=0; i<128; i++) words[i] = 64'hdeadbeefdeadbeef;
        for (int i=0; i<24; i++) words[i] = 0;
        words[1]=64'h1234; words[2]=128; words[3]={32'd13,32'd11};
        words[4]='h200; words[8]='h300;
        for (int i=0; i<12; i++) words[12+i]=words[i];
        for (int i=0; i<16; i++) words[64+i]=64'h1020304050607000+i;
        #100;
        fork
            begin @(negedge reg_clk); reg_rst_n=1; end
            begin @(negedge core_clk); core_rst_n=1; end
            begin @(negedge bus_clk); bus_rst_n=1; end
        join
        repeat (20) @(posedge reg_clk);
        configure();
        wait (completed_writes == 2);
        for (int i=0; i<16; i++)
            if (words[96+i] !== (64'h1020304050607000+i)) $fatal(1, "Copy mismatch");
        hold_aw=1; hold_b=1;
        write_reg(CHANNEL + pdma_rtl_ChannelBlockLayout::COMMAND_PRODUCER_OFFSET, 2);
        wait (memory.awvalid && w_seen);
        write_reg(RESET, 1);
        read_reg(RESET, value);
        if (value != 1 || !memory.awvalid) $fatal(1, "Reset retracted AW");
        @(negedge bus_clk); hold_aw=0;
        wait (b_pending);
        read_reg(RESET, value);
        if (value != 1) $fatal(1, "Reset completed before B");
        @(negedge bus_clk); hold_b=0;
        do read_reg(RESET, value); while (value != 0);
        if (completed_writes != 3) $fatal(1, "Reset issued additional writes");
        configure(); wait (completed_writes == 5);
        if (fatal || irq || awatop != 0) $fatal(1, "Unexpected fatal/IRQ/atomic");
        for (int i=0; i<16; i++)
            if (words[96+i] !== (64'h1020304050607000+i)) $fatal(1, "Post-reset copy mismatch");
        $display("PDMA_MINIMAL_SV_PASS");
        $finish;
    end
    initial begin #200000; $fatal(1, "Global test timeout"); end
endmodule
