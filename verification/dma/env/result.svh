typedef enum {TASK_SUCCESS, TASK_FAULT, TASK_ERROR, TASK_FATAL, TASK_RESET, TASK_ABORTED, IRQ_NOTIFICATION} pdma_outcome;

class pdma_result extends jvs_common_attr;
    int unsigned channel_id;
    logic [63:0] task_id, value, prefix;
    longint unsigned event_address;
    pdma_outcome outcome;
    int unsigned code, cause, event_producer;
    function new(string name="result"); super.new(name); endfunction
endclass
