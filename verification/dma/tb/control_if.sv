`include "configuration.svh"
interface pdma_control_if #(parameter int CHANNELS=`PDMA_CHANNEL_COUNT)(input logic reg_clk, core_clk, bus_clk);
    wire reg_rst_n, core_rst_n, bus_rst_n;
    wire [CHANNELS-1:0] irq;
    // Platform timebase, in core cycles, independent of DMA reset.
    bit timer_available=1;
    bit [63:0] timer_value=0;
    always @(posedge core_clk) timer_value <= timer_value + 1;
    bit maint_valid, maint_abort;
    bit accepted_abort;
    bit [31:0] maint_context;
    bit [CHANNELS-1:0] maint_channels;
    wire maint_ready;
    wire [2:0] maint_status;
    wire [CHANNELS-1:0] maint_failed;
    covergroup maintenance_results with function sample(bit abort_op,logic [2:0] status);
        operation: coverpoint abort_op;
        result: coverpoint status {
            bins success={pdma_rtl_MaintenanceState::Status_Success};
            bins bound_ring={pdma_rtl_MaintenanceState::Status_RingBound};
            bins channel_stopped={pdma_rtl_MaintenanceState::Status_ChannelStopped};
        }
        checked_results: cross operation,result;
    endgroup
    maintenance_results checked_maintenance=new;
    // Platform side of the core-clock maintenance handshake. Drive between
    // sampling edges so status and the retained bitmap can be checked together.
    task automatic request_maintenance(bit abort_op,bit [31:0] selector,bit [CHANNELS-1:0] scope);
        @(negedge core_clk);
        maint_context=selector; maint_channels=scope; maint_abort=abort_op; maint_valid=1;
        do @(posedge core_clk); while(maint_ready!==1);
        accepted_abort=abort_op;
        @(negedge core_clk);
        maint_valid=0;
    endtask
    task automatic expect_maintenance(bit [2:0] result,bit [CHANNELS-1:0] failed,int limit);
        repeat(limit) begin
            if(maint_ready===1) begin
                if(maint_status!==result || maint_failed!==failed)
                    $fatal(1,"Maintenance expected %0d/%h, observed %0d/%h",result,failed,maint_status,maint_failed);
                checked_maintenance.sample(accepted_abort,maint_status);
                return;
            end
            @(negedge core_clk);
        end
        $fatal(1,"Maintenance did not finish");
    endtask
    wire fatal, fatal_irq;
    logic [5:0] awatop;
endinterface
