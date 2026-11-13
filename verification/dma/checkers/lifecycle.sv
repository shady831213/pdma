module pdma_lifecycle_assertions(
    input logic clk, rst_n, request, ready, returned, accepted, closing, clear, enabled, has_commands,
    input logic irq, irq_ready, irq_source, parked, resume_request,
    input logic [3:0] request_slot, receipt_slot,
    input logic [1023:0] parked_state
);
    int unsigned outstanding=0;
    logic [15:0] owned='0;
    logic reset_quiet=0;
    always @(posedge clk or negedge rst_n) begin
        if(!rst_n) begin outstanding<=0; owned<='0; reset_quiet<=0; end
        else begin
            if(returned && accepted) begin
                assert(outstanding!=0 && owned[receipt_slot]) else $fatal(1,"DMA_UNOWNED_RECEIPT");
                owned[receipt_slot]<=0;
            end
            if(request && ready) begin
                // Read completion can forward a write for the same live chunk
                // at this edge. Old receipt consumption precedes new ownership.
                assert(!owned[request_slot] || (returned && accepted && request_slot==receipt_slot))
                    else $fatal(1,"DMA_DOUBLE_ACCEPT");
                owned[request_slot]<=1;
            end
            case ({request && ready,returned && accepted})
                2'b10: outstanding<=outstanding+1;
                2'b01: outstanding<=outstanding-1;
                default: ;
            endcase
            if(clear) reset_quiet<=1;
            else if(enabled && has_commands) reset_quiet<=0;
        end
    end
    assert property(@(posedge clk) disable iff(!rst_n) clear |-> !outstanding)
        else $fatal(1,"DMA_RESET_BEFORE_DRAIN");
    assert property(@(posedge clk) disable iff(!rst_n) closing |-> !request)
        else $fatal(1,"DMA_ADMISSION_DURING_RESET");
    assert property(@(posedge clk) disable iff(!rst_n) reset_quiet && !(enabled && has_commands) |-> !request)
        else $fatal(1,"DMA_ACCESS_AFTER_RESET");
    assert property(@(posedge clk) disable iff(!rst_n) irq && !irq_ready && !clear |=> irq || clear)
        else $fatal(1,"DMA_NOTIFICATION_DROPPED");
    assert property(@(posedge clk) disable iff(!rst_n) $rose(irq) |-> irq_source)
        else $fatal(1,"DMA_NOTIFICATION_WITHOUT_CAUSE");
    assert property(@(posedge clk) disable iff(!rst_n) parked && !resume_request && !closing |-> !request)
        else $fatal(1,"DMA_RETRY_WITHOUT_RESUME");
    assert property(@(posedge clk) disable iff(!rst_n)
        parked && !resume_request && !closing |=> $stable(parked_state) || closing || resume_request)
        else $fatal(1,"DMA_PARKED_STATE_CHANGED");
endmodule
