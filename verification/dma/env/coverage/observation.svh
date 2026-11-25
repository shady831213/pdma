class pdma_observation extends uvm_object;
    int unsigned channel_id, length, source_lane, dest_lane;
    longint unsigned task_id;
    logic [7:0] opcode;
    pdma_outcome outcome;
    realtime latency;
    bit cross_page, event_cross_page, fence_wide;
    bit source_sg, dest_sg, via_irq;
    function new(string name="observation"); super.new(name); endfunction
endclass
