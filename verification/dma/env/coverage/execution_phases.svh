// Sample consecutive registered phases of one owner. Using explicit pairs
// keeps reset/restart from joining two unrelated commands into an edge.
class pdma_execution_coverage;
    import pdma_rtl_CommandState::*;
    covergroup commands with function sample(Phase prior, Phase current);
        option.per_instance=1;
        edge_taken: coverpoint {prior,current} {
            bins linear_copy = {{Phase_Idle,Phase_Copy}};
            bins sg_copy = {{Phase_Idle,Phase_WalkSegments}};
            bins fence = {{Phase_Idle,Phase_FenceWrite}};
            bins reject = {{Phase_Idle,Phase_PrepareEvent}};
            bins fetch_fatal = {{Phase_Idle,Phase_Stopped}};
            bins span = {{Phase_WalkSegments,Phase_Copy}};
            bins list_error = {{Phase_WalkSegments,Phase_PrepareEvent}};
            bins next_span = {{Phase_Copy,Phase_WalkSegments}};
            bins copy_done = {{Phase_Copy,Phase_Idle}};
            bins payload_error = {{Phase_Copy,Phase_PrepareEvent}};
            bins fence_issued = {{Phase_FenceWrite,Phase_FenceReturn}};
            bins fence_done = {{Phase_FenceReturn,Phase_Idle}};
            bins fence_fault = {{Phase_FenceReturn,Phase_ParkFence}};
            bins fence_error = {{Phase_FenceReturn,Phase_PrepareEvent}};
            bins fence_resume = {{Phase_ParkFence,Phase_FenceWrite}};
            bins prepare = {{Phase_PrepareEvent,Phase_Publish}};
            bins unavailable_event = {{Phase_PrepareEvent,Phase_Stopped}};
            bins event_issued = {{Phase_Publish,Phase_PublishReturn}};
            bins event_fragment = {{Phase_PublishReturn,Phase_Publish}};
            bins event_finished = {{Phase_PublishReturn,Phase_Stopped}};
        }
    endgroup
    // The walker enum is local to the elaborated module. Accept its actual
    // literals so changing their encoding cannot silently relabel a bin.
    covergroup segments(int unsigned choose,locate,rd,receive_,decode_,offer_,payload)
            with function sample(int unsigned prior, int unsigned current);
        option.per_instance=1;
        edge_taken: coverpoint {prior,current} {
            bins locate_side = {{choose,locate}};
            bins buffered_pair = {{choose,offer_}};
            bins linear_endpoint = {{locate,decode_}};
            bins list_endpoint = {{locate,rd}};
            bins request = {{rd,receive_}};
            bins more_entry_bytes = {{receive_,rd}};
            bins entry_ready = {{receive_,decode_}};
            bins decoded = {{decode_,choose}};
            bins span_started = {{offer_,payload}};
            bins span_done = {{payload,choose}};
        }
    endgroup
    function new(string owner,int unsigned choose,locate,rd,receive_,decode_,offer_,payload);
        commands=new; commands.set_inst_name({owner,".command"});
        segments=new(choose,locate,rd,receive_,decode_,offer_,payload);
        segments.set_inst_name({owner,".segments"});
    endfunction
endclass
