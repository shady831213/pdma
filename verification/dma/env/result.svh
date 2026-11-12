typedef enum {TASK_SUCCESS, TASK_FAULT, TASK_ERROR, TASK_FATAL, TASK_RESET} pdma_outcome;

class pdma_result extends uvm_object;
    int unsigned channel_id;
    longint unsigned task_id, value, prefix;
    pdma_outcome outcome;
    int unsigned code, cause, event_producer;
    function new(string name="result"); super.new(name); endfunction
endclass
