# Reviewed 2026-11-12. See docs/rtl/throughput-review.md for source-level proofs.
# Exact source/tool/configuration binding; never refresh automatically to bypass findings.
# Grouped entries cover only proven unconnected/no-clock sinks, not observable corruption.
set identity [dict create rtl 6b0bf85820ae804a42bd8d620a787a5636a4b5d6859541531c10d58182cf52a3 design 73cc6f3e8707e7f36c62e4445744ff591ff754744281e32d0551a4dea2ff6d3b profile ad691b6cea5e534668942db955a580e6947f14a4010e91062a5a4ddb6092001d constraints fc70d90de5d746a6ef316c8f0d208067e61d1d93cd7127d8084e4c22afb49464 common edc6584104dba47ecbe8cf5c21b4e87b040cd008c39a4d319a64229471e9839e analysis 52bfb99394dd148bb38cfb0e91fe2fbaf491daf0c5bf6c1e65c644c1fa0bde4b tool aec2fec538b2a0e09d2c9aa7cf17f060d8f3dc1436fa1384fe212e2faaf37761]
set findings {}
lappend findings [dict create \
    checks [list {cdc}] tag {CDC_COHERENCY_RECONV_COMB} \
    reason {G1: registered one-step Gray pointers through two stages; bounded crossing delay below the source period; local stages remain normally timed.} \
    fields [dict create \
        {ConvergencePoint} [list {request_crossing/queue/u_s_gray2bin/g_base.bot_red[0]}] \
        {ConvControlPathList:ConvergingControlPath:SyncOutput} [list {request_crossing/queue/u_synchronizer_rp/g[3].rg/Q[1]} {request_crossing/queue/u_synchronizer_rp/g[2].rg/Q[1]} {request_crossing/queue/u_synchronizer_rp/g[1].rg/Q[1]} {request_crossing/queue/u_synchronizer_rp/g[0].rg/Q[1]}] \
    ]]
lappend findings [dict create \
    checks [list {cdc}] tag {CDC_COHERENCY_RECONV_COMB} \
    reason {G1: registered one-step Gray pointers through two stages; bounded crossing delay below the source period; local stages remain normally timed.} \
    fields [dict create \
        {ConvergencePoint} [list {response_crossing/queue/u_d_gray2bin/g_base.bot_red[0]}] \
        {ConvControlPathList:ConvergingControlPath:SyncOutput} [list {response_crossing/queue/u_synchronizer_wp/g[3].rg/Q[1]} {response_crossing/queue/u_synchronizer_wp/g[2].rg/Q[1]} {response_crossing/queue/u_synchronizer_wp/g[1].rg/Q[1]} {response_crossing/queue/u_synchronizer_wp/g[0].rg/Q[1]}] \
    ]]
lappend findings [dict create \
    checks [list {cdc}] tag {CDC_COHERENCY_RECONV_COMB} \
    reason {G1: registered one-step Gray pointers through two stages; bounded crossing delay below the source period; local stages remain normally timed.} \
    fields [dict create \
        {ConvergencePoint} [list {request_crossing/queue/u_d_gray2bin/g_base.bot_red[0]}] \
        {ConvControlPathList:ConvergingControlPath:SyncOutput} [list {request_crossing/queue/u_synchronizer_wp/g[3].rg/Q[1]} {request_crossing/queue/u_synchronizer_wp/g[2].rg/Q[1]} {request_crossing/queue/u_synchronizer_wp/g[1].rg/Q[1]} {request_crossing/queue/u_synchronizer_wp/g[0].rg/Q[1]}] \
    ]]
lappend findings [dict create \
    checks [list {cdc}] tag {CDC_COHERENCY_RECONV_COMB} \
    reason {G1: registered one-step Gray pointers through two stages; bounded crossing delay below the source period; local stages remain normally timed.} \
    fields [dict create \
        {ConvergencePoint} [list {response_crossing/queue/u_s_gray2bin/g_base.bot_red[0]}] \
        {ConvControlPathList:ConvergingControlPath:SyncOutput} [list {response_crossing/queue/u_synchronizer_rp/g[3].rg/Q[1]} {response_crossing/queue/u_synchronizer_rp/g[2].rg/Q[1]} {response_crossing/queue/u_synchronizer_rp/g[1].rg/Q[1]} {response_crossing/queue/u_synchronizer_rp/g[0].rg/Q[1]}] \
    ]]
lappend findings [dict create \
    checks [list {cdc}] tag {CDC_COHERENCY_RECONV_SEQ} \
    reason {C1: independent request/receipt/control credits need no simultaneous arrival; valid custody, per-slot debt and reset Close prevent overwrite or early reuse.} \
    fields [dict create \
        {ConvergencePoint} [list {csr/crossing/requests/g_dst_side.valid/D}] \
        {ConvControlPathList:ConvergingControlPath:SyncOutput} [list {csr/crossing/requests/u_s_to_d/g[0].rg/Q[1]} {csr/crossing/responses/u_d_to_s/g[0].rg/Q[1]}] \
    ]]
lappend findings [dict create \
    checks [list {cdc}] tag {CDC_COHERENCY_RECONV_SEQ} \
    reason {C1: independent request/receipt/control credits need no simultaneous arrival; valid custody, per-slot debt and reset Close prevent overwrite or early reuse.} \
    fields [dict create \
        {ConvergencePoint} [list {endpoint/reader/pending/u_controller/update_state}] \
        {ConvControlPathList:ConvergingControlPath:SyncOutput} [list {request_crossing/queue/u_synchronizer_wp/g[3].rg/Q[1]} {request_crossing/queue/u_synchronizer_wp/g[2].rg/Q[1]} {request_crossing/queue/u_synchronizer_wp/g[1].rg/Q[1]} {request_crossing/queue/u_synchronizer_wp/g[0].rg/Q[1]} {response_crossing/queue/u_synchronizer_rp/g[3].rg/Q[1]} {response_crossing/queue/u_synchronizer_rp/g[2].rg/Q[1]} {response_crossing/queue/u_synchronizer_rp/g[1].rg/Q[1]} {response_crossing/queue/u_synchronizer_rp/g[0].rg/Q[1]}] \
    ]]
lappend findings [dict create \
    checks [list {cdc}] tag {CDC_COHERENCY_ASYNCSRCS_RECONV_SEQ} \
    reason {C1: independent request/receipt/control credits need no simultaneous arrival; valid custody, per-slot debt and reset Close prevent overwrite or early reuse.} \
    fields [dict create \
        {ConvergencePoint} [list {request_crossing/queue/word_count[3]}] \
        {ConvControlPathList:ConvergingControlPath:SyncOutput} [list {csr/crossing/requests/u_s_to_d/g[0].rg/Q[1]} {csr/crossing/responses/u_d_to_s/g[0].rg/Q[1]} {request_crossing/queue/u_synchronizer_rp/g[3].rg/Q[1]} {request_crossing/queue/u_synchronizer_rp/g[2].rg/Q[1]} {request_crossing/queue/u_synchronizer_rp/g[1].rg/Q[1]} {request_crossing/queue/u_synchronizer_rp/g[0].rg/Q[1]} {response_crossing/queue/u_synchronizer_wp/g[3].rg/Q[1]} {response_crossing/queue/u_synchronizer_wp/g[2].rg/Q[1]} {response_crossing/queue/u_synchronizer_wp/g[1].rg/Q[1]} {response_crossing/queue/u_synchronizer_wp/g[0].rg/Q[1]}] \
    ]]
lappend findings [dict create \
    checks [list {cdc} {rdc}] tag {SETUP_RESET_DRIVING_NON_ASYNC_PIN} \
    reason {R1: coordinated cold reset invalidates slots and queue custody; fresh data precedes valid; software reset drains without resetting transport.} \
    fields [dict create \
        {ResetName} [list {core_rst_n}] \
        {NonResetInstance} [list {engine/copy/chunks[0].source[0:63]}] \
    ]]
lappend findings [dict create \
    checks [list {cdc} {rdc}] tag {SETUP_RESET_DRIVING_NON_ASYNC_PIN} \
    reason {R1: coordinated cold reset invalidates slots and queue custody; fresh data precedes valid; software reset drains without resetting transport.} \
    fields [dict create \
        {ResetName} [list {core_rst_n}] \
        {NonResetInstance} [list {engine/copy/chunks[3].source[0:63]}] \
    ]]
lappend findings [dict create \
    checks [list {cdc} {rdc}] tag {SETUP_RESET_DRIVING_NON_ASYNC_PIN} \
    reason {R1: coordinated cold reset invalidates slots and queue custody; fresh data precedes valid; software reset drains without resetting transport.} \
    fields [dict create \
        {ResetName} [list {core_rst_n}] \
        {NonResetInstance} [list {engine/copy/chunks[2].source[0:63]}] \
    ]]
lappend findings [dict create \
    checks [list {cdc} {rdc}] tag {SETUP_RESET_DRIVING_NON_ASYNC_PIN} \
    reason {R1: coordinated cold reset invalidates slots and queue custody; fresh data precedes valid; software reset drains without resetting transport.} \
    fields [dict create \
        {ResetName} [list {core_rst_n}] \
        {NonResetInstance} [list {engine/copy/chunks[6].destination[0:63]}] \
    ]]
lappend findings [dict create \
    checks [list {cdc} {rdc}] tag {SETUP_RESET_DRIVING_NON_ASYNC_PIN} \
    reason {R1: coordinated cold reset invalidates slots and queue custody; fresh data precedes valid; software reset drains without resetting transport.} \
    fields [dict create \
        {ResetName} [list {core_rst_n}] \
        {NonResetInstance} [list engine/copy/chunks\[4\].length\{ {[0]} {,[3:6]} \}] \
    ]]
lappend findings [dict create \
    checks [list {cdc} {rdc}] tag {SETUP_RESET_DRIVING_NON_ASYNC_PIN} \
    reason {R1: coordinated cold reset invalidates slots and queue custody; fresh data precedes valid; software reset drains without resetting transport.} \
    fields [dict create \
        {ResetName} [list {core_rst_n}] \
        {NonResetInstance} [list {engine/copy/chunks[6].source[0:63]}] \
    ]]
lappend findings [dict create \
    checks [list {cdc} {rdc}] tag {SETUP_RESET_DRIVING_NON_ASYNC_PIN} \
    reason {R1: coordinated cold reset invalidates slots and queue custody; fresh data precedes valid; software reset drains without resetting transport.} \
    fields [dict create \
        {ResetName} [list {core_rst_n}] \
        {NonResetInstance} [list engine/copy/chunks\[5\].length\{ {[0]} {,[3:6]} \}] \
    ]]
lappend findings [dict create \
    checks [list {cdc} {rdc}] tag {SETUP_RESET_DRIVING_NON_ASYNC_PIN} \
    reason {R1: coordinated cold reset invalidates slots and queue custody; fresh data precedes valid; software reset drains without resetting transport.} \
    fields [dict create \
        {ResetName} [list {core_rst_n}] \
        {NonResetInstance} [list {engine/copy/chunks[7].source[0:63]}] \
    ]]
lappend findings [dict create \
    checks [list {cdc} {rdc}] tag {SETUP_RESET_DRIVING_NON_ASYNC_PIN} \
    reason {R1: coordinated cold reset invalidates slots and queue custody; fresh data precedes valid; software reset drains without resetting transport.} \
    fields [dict create \
        {ResetName} [list {core_rst_n}] \
        {NonResetInstance} [list {engine/copy/chunks[3].destination[0:63]}] \
    ]]
lappend findings [dict create \
    checks [list {cdc} {rdc}] tag {SETUP_RESET_DRIVING_NON_ASYNC_PIN} \
    reason {R1: coordinated cold reset invalidates slots and queue custody; fresh data precedes valid; software reset drains without resetting transport.} \
    fields [dict create \
        {ResetName} [list {core_rst_n}] \
        {NonResetInstance} [list engine/copy/chunks\[3\].length\{ {[0]} {,[3:6]} \}] \
    ]]
lappend findings [dict create \
    checks [list {cdc} {rdc}] tag {SETUP_RESET_DRIVING_NON_ASYNC_PIN} \
    reason {R1: coordinated cold reset invalidates slots and queue custody; fresh data precedes valid; software reset drains without resetting transport.} \
    fields [dict create \
        {ResetName} [list {core_rst_n}] \
        {NonResetInstance} [list {engine/copy/chunks[4].source[0:63]}] \
    ]]
lappend findings [dict create \
    checks [list {cdc} {rdc}] tag {SETUP_RESET_DRIVING_NON_ASYNC_PIN} \
    reason {R1: coordinated cold reset invalidates slots and queue custody; fresh data precedes valid; software reset drains without resetting transport.} \
    fields [dict create \
        {ResetName} [list {core_rst_n}] \
        {NonResetInstance} [list {engine/copy/chunks[1].source[0:63]}] \
    ]]
lappend findings [dict create \
    checks [list {cdc} {rdc}] tag {SETUP_RESET_DRIVING_NON_ASYNC_PIN} \
    reason {R1: coordinated cold reset invalidates slots and queue custody; fresh data precedes valid; software reset drains without resetting transport.} \
    fields [dict create \
        {ResetName} [list {core_rst_n}] \
        {NonResetInstance} [list engine/copy/chunks\[6\].length\{ {[0]} {,[3:6]} \}] \
    ]]
lappend findings [dict create \
    checks [list {cdc} {rdc}] tag {SETUP_RESET_DRIVING_NON_ASYNC_PIN} \
    reason {R1: coordinated cold reset invalidates slots and queue custody; fresh data precedes valid; software reset drains without resetting transport.} \
    fields [dict create \
        {ResetName} [list {core_rst_n}] \
        {NonResetInstance} [list {engine/copy/chunks[0].destination[0:63]}] \
    ]]
lappend findings [dict create \
    checks [list {cdc} {rdc}] tag {SETUP_RESET_DRIVING_NON_ASYNC_PIN} \
    reason {R1: coordinated cold reset invalidates slots and queue custody; fresh data precedes valid; software reset drains without resetting transport.} \
    fields [dict create \
        {ResetName} [list {core_rst_n}] \
        {NonResetInstance} [list engine/copy/chunks\[2\].length\{ {[0]} {,[3:6]} \}] \
    ]]
lappend findings [dict create \
    checks [list {cdc} {rdc}] tag {SETUP_RESET_DRIVING_NON_ASYNC_PIN} \
    reason {R1: coordinated cold reset invalidates slots and queue custody; fresh data precedes valid; software reset drains without resetting transport.} \
    fields [dict create \
        {ResetName} [list {core_rst_n}] \
        {NonResetInstance} [list {engine/copy/chunks[5].destination[0:63]}] \
    ]]
lappend findings [dict create \
    checks [list {cdc} {rdc}] tag {SETUP_RESET_DRIVING_NON_ASYNC_PIN} \
    reason {R1: coordinated cold reset invalidates slots and queue custody; fresh data precedes valid; software reset drains without resetting transport.} \
    fields [dict create \
        {ResetName} [list {core_rst_n}] \
        {NonResetInstance} [list {engine/copy/chunks[4].destination[0:63]}] \
    ]]
lappend findings [dict create \
    checks [list {cdc} {rdc}] tag {SETUP_RESET_DRIVING_NON_ASYNC_PIN} \
    reason {R1: coordinated cold reset invalidates slots and queue custody; fresh data precedes valid; software reset drains without resetting transport.} \
    fields [dict create \
        {ResetName} [list {core_rst_n}] \
        {NonResetInstance} [list {engine/copy/chunks[5].source[0:63]}] \
    ]]
lappend findings [dict create \
    checks [list {cdc} {rdc}] tag {SETUP_RESET_DRIVING_NON_ASYNC_PIN} \
    reason {R1: coordinated cold reset invalidates slots and queue custody; fresh data precedes valid; software reset drains without resetting transport.} \
    fields [dict create \
        {ResetName} [list {core_rst_n}] \
        {NonResetInstance} [list engine/copy/chunks\[1\].length\{ {[0]} {,[3:6]} \}] \
    ]]
lappend findings [dict create \
    checks [list {cdc} {rdc}] tag {SETUP_RESET_DRIVING_NON_ASYNC_PIN} \
    reason {R1: coordinated cold reset invalidates slots and queue custody; fresh data precedes valid; software reset drains without resetting transport.} \
    fields [dict create \
        {ResetName} [list {core_rst_n}] \
        {NonResetInstance} [list engine/copy/chunks\[7\].length\{ {[0]} {,[3:6]} \}] \
    ]]
lappend findings [dict create \
    checks [list {cdc} {rdc}] tag {SETUP_RESET_DRIVING_NON_ASYNC_PIN} \
    reason {R1: coordinated cold reset invalidates slots and queue custody; fresh data precedes valid; software reset drains without resetting transport.} \
    fields [dict create \
        {ResetName} [list {core_rst_n}] \
        {NonResetInstance} [list engine/copy/chunks\[0\].length\{ {[0]} {,[3:6]} \}] \
    ]]
lappend findings [dict create \
    checks [list {cdc} {rdc}] tag {SETUP_RESET_DRIVING_NON_ASYNC_PIN} \
    reason {R1: coordinated cold reset invalidates slots and queue custody; fresh data precedes valid; software reset drains without resetting transport.} \
    fields [dict create \
        {ResetName} [list {core_rst_n}] \
        {NonResetInstance} [list {engine/copy/chunks[1].destination[0:63]}] \
    ]]
lappend findings [dict create \
    checks [list {cdc} {rdc}] tag {SETUP_RESET_DRIVING_NON_ASYNC_PIN} \
    reason {R1: coordinated cold reset invalidates slots and queue custody; fresh data precedes valid; software reset drains without resetting transport.} \
    fields [dict create \
        {ResetName} [list {core_rst_n}] \
        {NonResetInstance} [list {engine/copy/chunks[7].destination[0:63]}] \
    ]]
lappend findings [dict create \
    checks [list {cdc} {rdc}] tag {SETUP_RESET_DRIVING_NON_ASYNC_PIN} \
    reason {R1: coordinated cold reset invalidates slots and queue custody; fresh data precedes valid; software reset drains without resetting transport.} \
    fields [dict create \
        {ResetName} [list {core_rst_n}] \
        {NonResetInstance} [list {engine/copy/chunks[2].destination[0:63]}] \
    ]]
lappend findings [dict create \
    checks [list {cdc} {rdc}] tag {SETUP_RESET_DRIVING_NON_ASYNC_PIN} \
    reason {R1: coordinated cold reset invalidates slots and queue custody; fresh data precedes valid; software reset drains without resetting transport.} \
    fields [dict create \
        {ResetName} [list {core_rst_n}] \
        {NonResetInstance} [list {engine/copy/buffers/raw_read.data[511:0]}] \
    ]]
lappend findings [dict create \
    checks [list {cdc} {rdc}] tag {SETUP_RESET_DRIVING_NON_ASYNC_PIN} \
    reason {R1: coordinated cold reset invalidates slots and queue custody; fresh data precedes valid; software reset drains without resetting transport.} \
    fields [dict create \
        {ResetName} [list {core_rst_n}] \
        {NonResetInstance} [list request_crossing/queue/ram\{ {[0:7][618:518]} {,[0:7][516:0]} \}] \
    ]]
lappend findings [dict create \
    checks [list {cdc} {rdc}] tag {SETUP_RESET_DRIVING_NON_ASYNC_PIN} \
    reason {R1: coordinated cold reset invalidates slots and queue custody; fresh data precedes valid; software reset drains without resetting transport.} \
    fields [dict create \
        {ResetName} [list {reg_rst_n}] \
        {NonResetInstance} [list {csr/frontend/aw_storage/g_forward.data.address[31:0]}] \
    ]]
lappend findings [dict create \
    checks [list {cdc} {rdc}] tag {SETUP_RESET_DRIVING_NON_ASYNC_PIN} \
    reason {R1: coordinated cold reset invalidates slots and queue custody; fresh data precedes valid; software reset drains without resetting transport.} \
    fields [dict create \
        {ResetName} [list {reg_rst_n}] \
        {NonResetInstance} [list {csr/frontend/aw_storage/g_forward.data.identity[0]}] \
    ]]
lappend findings [dict create \
    checks [list {cdc} {rdc}] tag {SETUP_RESET_DRIVING_NON_ASYNC_PIN} \
    reason {R1: coordinated cold reset invalidates slots and queue custody; fresh data precedes valid; software reset drains without resetting transport.} \
    fields [dict create \
        {ResetName} [list {reg_rst_n}] \
        {NonResetInstance} [list {csr/frontend/w_storage/g_forward.data.data[31:0]}] \
    ]]
lappend findings [dict create \
    checks [list {cdc} {rdc}] tag {SETUP_RESET_DRIVING_NON_ASYNC_PIN} \
    reason {R1: coordinated cold reset invalidates slots and queue custody; fresh data precedes valid; software reset drains without resetting transport.} \
    fields [dict create \
        {ResetName} [list {reg_rst_n}] \
        {NonResetInstance} [list {csr/frontend/w_storage/g_forward.data.strobe[3:0]}] \
    ]]
lappend findings [dict create \
    checks [list {cdc} {rdc}] tag {SETUP_RESET_DRIVING_NON_ASYNC_PIN} \
    reason {R1: coordinated cold reset invalidates slots and queue custody; fresh data precedes valid; software reset drains without resetting transport.} \
    fields [dict create \
        {ResetName} [list {reg_rst_n}] \
        {NonResetInstance} [list {csr/frontend/ar_storage/g_forward.data.address[31:0]}] \
    ]]
lappend findings [dict create \
    checks [list {cdc} {rdc}] tag {SETUP_RESET_DRIVING_NON_ASYNC_PIN} \
    reason {R1: coordinated cold reset invalidates slots and queue custody; fresh data precedes valid; software reset drains without resetting transport.} \
    fields [dict create \
        {ResetName} [list {reg_rst_n}] \
        {NonResetInstance} [list {csr/frontend/ar_storage/g_forward.data.identity[0]}] \
    ]]
lappend findings [dict create \
    checks [list {cdc} {rdc}] tag {INTEGRITY_RESET_ASYNC_AND_SYNC_USAGE} \
    reason {R2: reset-controlled RAM read enable changes invalid data, not state reset semantics; prepared-valid resets and captures only a completed synchronous read.} \
    fields [dict create \
        {ResetObject} [list {core_rst_n}] \
        {Instance1} [list {irq}] \
        {Instance2} [list {engine/copy/buffers/u_ram/read_port.data[0]}] \
        {ResetType1} [list {Asynchronous}] \
        {ResetType2} [list {synchronously}] \
    ]]
lappend findings [dict create \
    checks [list {rdc}] tag {RDC_CORRUPT_OBSERVED} \
    reason {D1: exact unconnected/no-clock sink. FIFO controller i_data is unused with MATCH_COUNT_WIDTH=0; RAM collision is unused with FORWARD_NEW_DATA=false. No sequential or output consumer. Count is coverage only.} \
    fields [dict create \
        {DestObject} [list {endpoint/reader/pending/u_controller/i_data.lane[2:0]}] \
        {DestObjectType} [list {unconnected}] \
        {RdcSourceResets:ResetName} [list {bus_rst_n}] \
        {DestClockInfoList:DestClockInfo:ClockName} [list {'no-clock'}] \
    ] expected_count 41]
lappend findings [dict create \
    checks [list {rdc}] tag {RDC_CORRUPT_OBSERVED} \
    reason {D1: exact unconnected/no-clock sink. FIFO controller i_data is unused with MATCH_COUNT_WIDTH=0; RAM collision is unused with FORWARD_NEW_DATA=false. No sequential or output consumer. Count is coverage only.} \
    fields [dict create \
        {DestObject} [list {endpoint/reader/pending/u_controller/i_data.lane[2:0]}] \
        {DestObjectType} [list {unconnected}] \
        {RdcSourceResets:ResetName} [list {core_rst_n}] \
        {DestClockInfoList:DestClockInfo:ClockName} [list {'no-clock'}] \
    ] expected_count 54]
lappend findings [dict create \
    checks [list {rdc}] tag {RDC_CORRUPT_OBSERVED} \
    reason {D1: exact unconnected/no-clock sink. FIFO controller i_data is unused with MATCH_COUNT_WIDTH=0; RAM collision is unused with FORWARD_NEW_DATA=false. No sequential or output consumer. Count is coverage only.} \
    fields [dict create \
        {DestObject} [list {endpoint/reader/pending/u_controller/i_data.length[2:0]}] \
        {DestObjectType} [list {unconnected}] \
        {RdcSourceResets:ResetName} [list {bus_rst_n}] \
        {DestClockInfoList:DestClockInfo:ClockName} [list {'no-clock'}] \
    ] expected_count 41]
lappend findings [dict create \
    checks [list {rdc}] tag {RDC_CORRUPT_OBSERVED} \
    reason {D1: exact unconnected/no-clock sink. FIFO controller i_data is unused with MATCH_COUNT_WIDTH=0; RAM collision is unused with FORWARD_NEW_DATA=false. No sequential or output consumer. Count is coverage only.} \
    fields [dict create \
        {DestObject} [list {endpoint/reader/pending/u_controller/i_data.length[2:0]}] \
        {DestObjectType} [list {unconnected}] \
        {RdcSourceResets:ResetName} [list {core_rst_n}] \
        {DestClockInfoList:DestClockInfo:ClockName} [list {'no-clock'}] \
    ] expected_count 53]
lappend findings [dict create \
    checks [list {rdc}] tag {RDC_CORRUPT_OBSERVED} \
    reason {D1: exact unconnected/no-clock sink. FIFO controller i_data is unused with MATCH_COUNT_WIDTH=0; RAM collision is unused with FORWARD_NEW_DATA=false. No sequential or output consumer. Count is coverage only.} \
    fields [dict create \
        {DestObject} [list {endpoint/reader/pending/u_controller/i_data.size[0]}] \
        {DestObjectType} [list {unconnected}] \
        {RdcSourceResets:ResetName} [list {core_rst_n}] \
        {DestClockInfoList:DestClockInfo:ClockName} [list {'no-clock'}] \
    ] expected_count 1]
lappend findings [dict create \
    checks [list {rdc}] tag {RDC_CORRUPT_OBSERVED} \
    reason {D1: exact unconnected/no-clock sink. FIFO controller i_data is unused with MATCH_COUNT_WIDTH=0; RAM collision is unused with FORWARD_NEW_DATA=false. No sequential or output consumer. Count is coverage only.} \
    fields [dict create \
        {DestObject} [list {endpoint/reader/pending/u_controller/i_data.size[1:0]}] \
        {DestObjectType} [list {unconnected}] \
        {RdcSourceResets:ResetName} [list {bus_rst_n}] \
        {DestClockInfoList:DestClockInfo:ClockName} [list {'no-clock'}] \
    ] expected_count 41]
lappend findings [dict create \
    checks [list {rdc}] tag {RDC_CORRUPT_OBSERVED} \
    reason {D1: exact unconnected/no-clock sink. FIFO controller i_data is unused with MATCH_COUNT_WIDTH=0; RAM collision is unused with FORWARD_NEW_DATA=false. No sequential or output consumer. Count is coverage only.} \
    fields [dict create \
        {DestObject} [list {endpoint/reader/pending/u_controller/i_data.size[1:0]}] \
        {DestObjectType} [list {unconnected}] \
        {RdcSourceResets:ResetName} [list {core_rst_n}] \
        {DestClockInfoList:DestClockInfo:ClockName} [list {'no-clock'}] \
    ] expected_count 53]
lappend findings [dict create \
    checks [list {rdc}] tag {RDC_CORRUPT_OBSERVED} \
    reason {D1: exact unconnected/no-clock sink. FIFO controller i_data is unused with MATCH_COUNT_WIDTH=0; RAM collision is unused with FORWARD_NEW_DATA=false. No sequential or output consumer. Count is coverage only.} \
    fields [dict create \
        {DestObject} [list {endpoint/reader/pending/u_controller/i_data.slot[3:0]}] \
        {DestObjectType} [list {unconnected}] \
        {RdcSourceResets:ResetName} [list {bus_rst_n}] \
        {DestClockInfoList:DestClockInfo:ClockName} [list {'no-clock'}] \
    ] expected_count 41]
lappend findings [dict create \
    checks [list {rdc}] tag {RDC_CORRUPT_OBSERVED} \
    reason {D1: exact unconnected/no-clock sink. FIFO controller i_data is unused with MATCH_COUNT_WIDTH=0; RAM collision is unused with FORWARD_NEW_DATA=false. No sequential or output consumer. Count is coverage only.} \
    fields [dict create \
        {DestObject} [list {endpoint/reader/pending/u_controller/i_data.slot[3:0]}] \
        {DestObjectType} [list {unconnected}] \
        {RdcSourceResets:ResetName} [list {core_rst_n}] \
        {DestClockInfoList:DestClockInfo:ClockName} [list {'no-clock'}] \
    ] expected_count 53]
lappend findings [dict create \
    checks [list {rdc}] tag {RDC_CORRUPT_OBSERVED} \
    reason {D1: exact unconnected/no-clock sink. FIFO controller i_data is unused with MATCH_COUNT_WIDTH=0; RAM collision is unused with FORWARD_NEW_DATA=false. No sequential or output consumer. Count is coverage only.} \
    fields [dict create \
        {DestObject} [list {endpoint/writer/addresses/u_controller/i_data.address[63:0]}] \
        {DestObjectType} [list {unconnected}] \
        {RdcSourceResets:ResetName} [list {bus_rst_n}] \
        {DestClockInfoList:DestClockInfo:ClockName} [list {'no-clock'}] \
    ] expected_count 41]
lappend findings [dict create \
    checks [list {rdc}] tag {RDC_CORRUPT_OBSERVED} \
    reason {D1: exact unconnected/no-clock sink. FIFO controller i_data is unused with MATCH_COUNT_WIDTH=0; RAM collision is unused with FORWARD_NEW_DATA=false. No sequential or output consumer. Count is coverage only.} \
    fields [dict create \
        {DestObject} [list {endpoint/writer/addresses/u_controller/i_data.address[63:0]}] \
        {DestObjectType} [list {unconnected}] \
        {RdcSourceResets:ResetName} [list {core_rst_n}] \
        {DestClockInfoList:DestClockInfo:ClockName} [list {'no-clock'}] \
    ] expected_count 54]
lappend findings [dict create \
    checks [list {rdc}] tag {RDC_CORRUPT_OBSERVED} \
    reason {D1: exact unconnected/no-clock sink. FIFO controller i_data is unused with MATCH_COUNT_WIDTH=0; RAM collision is unused with FORWARD_NEW_DATA=false. No sequential or output consumer. Count is coverage only.} \
    fields [dict create \
        {DestObject} [list {endpoint/writer/addresses/u_controller/i_data.length[2:0]}] \
        {DestObjectType} [list {unconnected}] \
        {RdcSourceResets:ResetName} [list {bus_rst_n}] \
        {DestClockInfoList:DestClockInfo:ClockName} [list {'no-clock'}] \
    ] expected_count 41]
lappend findings [dict create \
    checks [list {rdc}] tag {RDC_CORRUPT_OBSERVED} \
    reason {D1: exact unconnected/no-clock sink. FIFO controller i_data is unused with MATCH_COUNT_WIDTH=0; RAM collision is unused with FORWARD_NEW_DATA=false. No sequential or output consumer. Count is coverage only.} \
    fields [dict create \
        {DestObject} [list {endpoint/writer/addresses/u_controller/i_data.length[2:0]}] \
        {DestObjectType} [list {unconnected}] \
        {RdcSourceResets:ResetName} [list {core_rst_n}] \
        {DestClockInfoList:DestClockInfo:ClockName} [list {'no-clock'}] \
    ] expected_count 53]
lappend findings [dict create \
    checks [list {rdc}] tag {RDC_CORRUPT_OBSERVED} \
    reason {D1: exact unconnected/no-clock sink. FIFO controller i_data is unused with MATCH_COUNT_WIDTH=0; RAM collision is unused with FORWARD_NEW_DATA=false. No sequential or output consumer. Count is coverage only.} \
    fields [dict create \
        {DestObject} [list {endpoint/writer/addresses/u_controller/i_data.selector[31:0]}] \
        {DestObjectType} [list {unconnected}] \
        {RdcSourceResets:ResetName} [list {bus_rst_n}] \
        {DestClockInfoList:DestClockInfo:ClockName} [list {'no-clock'}] \
    ] expected_count 41]
lappend findings [dict create \
    checks [list {rdc}] tag {RDC_CORRUPT_OBSERVED} \
    reason {D1: exact unconnected/no-clock sink. FIFO controller i_data is unused with MATCH_COUNT_WIDTH=0; RAM collision is unused with FORWARD_NEW_DATA=false. No sequential or output consumer. Count is coverage only.} \
    fields [dict create \
        {DestObject} [list {endpoint/writer/addresses/u_controller/i_data.selector[31:0]}] \
        {DestObjectType} [list {unconnected}] \
        {RdcSourceResets:ResetName} [list {core_rst_n}] \
        {DestClockInfoList:DestClockInfo:ClockName} [list {'no-clock'}] \
    ] expected_count 54]
lappend findings [dict create \
    checks [list {rdc}] tag {RDC_CORRUPT_OBSERVED} \
    reason {D1: exact unconnected/no-clock sink. FIFO controller i_data is unused with MATCH_COUNT_WIDTH=0; RAM collision is unused with FORWARD_NEW_DATA=false. No sequential or output consumer. Count is coverage only.} \
    fields [dict create \
        {DestObject} [list {endpoint/writer/addresses/u_controller/i_data.size[0]}] \
        {DestObjectType} [list {unconnected}] \
        {RdcSourceResets:ResetName} [list {core_rst_n}] \
        {DestClockInfoList:DestClockInfo:ClockName} [list {'no-clock'}] \
    ] expected_count 1]
lappend findings [dict create \
    checks [list {rdc}] tag {RDC_CORRUPT_OBSERVED} \
    reason {D1: exact unconnected/no-clock sink. FIFO controller i_data is unused with MATCH_COUNT_WIDTH=0; RAM collision is unused with FORWARD_NEW_DATA=false. No sequential or output consumer. Count is coverage only.} \
    fields [dict create \
        {DestObject} [list {endpoint/writer/addresses/u_controller/i_data.size[1:0]}] \
        {DestObjectType} [list {unconnected}] \
        {RdcSourceResets:ResetName} [list {bus_rst_n}] \
        {DestClockInfoList:DestClockInfo:ClockName} [list {'no-clock'}] \
    ] expected_count 41]
lappend findings [dict create \
    checks [list {rdc}] tag {RDC_CORRUPT_OBSERVED} \
    reason {D1: exact unconnected/no-clock sink. FIFO controller i_data is unused with MATCH_COUNT_WIDTH=0; RAM collision is unused with FORWARD_NEW_DATA=false. No sequential or output consumer. Count is coverage only.} \
    fields [dict create \
        {DestObject} [list {endpoint/writer/addresses/u_controller/i_data.size[1:0]}] \
        {DestObjectType} [list {unconnected}] \
        {RdcSourceResets:ResetName} [list {core_rst_n}] \
        {DestClockInfoList:DestClockInfo:ClockName} [list {'no-clock'}] \
    ] expected_count 53]
lappend findings [dict create \
    checks [list {rdc}] tag {RDC_CORRUPT_OBSERVED} \
    reason {D1: exact unconnected/no-clock sink. FIFO controller i_data is unused with MATCH_COUNT_WIDTH=0; RAM collision is unused with FORWARD_NEW_DATA=false. No sequential or output consumer. Count is coverage only.} \
    fields [dict create \
        {DestObject} [list {endpoint/writer/data/u_controller/i_data.data[191:0]}] \
        {DestObjectType} [list {unconnected}] \
        {RdcSourceResets:ResetName} [list {core_rst_n}] \
        {DestClockInfoList:DestClockInfo:ClockName} [list {'no-clock'}] \
    ] expected_count 1]
lappend findings [dict create \
    checks [list {rdc}] tag {RDC_CORRUPT_OBSERVED} \
    reason {D1: exact unconnected/no-clock sink. FIFO controller i_data is unused with MATCH_COUNT_WIDTH=0; RAM collision is unused with FORWARD_NEW_DATA=false. No sequential or output consumer. Count is coverage only.} \
    fields [dict create \
        {DestObject} [list {endpoint/writer/data/u_controller/i_data.data[511:0]}] \
        {DestObjectType} [list {unconnected}] \
        {RdcSourceResets:ResetName} [list {bus_rst_n}] \
        {DestClockInfoList:DestClockInfo:ClockName} [list {'no-clock'}] \
    ] expected_count 43]
lappend findings [dict create \
    checks [list {rdc}] tag {RDC_CORRUPT_OBSERVED} \
    reason {D1: exact unconnected/no-clock sink. FIFO controller i_data is unused with MATCH_COUNT_WIDTH=0; RAM collision is unused with FORWARD_NEW_DATA=false. No sequential or output consumer. Count is coverage only.} \
    fields [dict create \
        {DestObject} [list {endpoint/writer/data/u_controller/i_data.data[511:0]}] \
        {DestObjectType} [list {unconnected}] \
        {RdcSourceResets:ResetName} [list {core_rst_n}] \
        {DestClockInfoList:DestClockInfo:ClockName} [list {'no-clock'}] \
    ] expected_count 54]
lappend findings [dict create \
    checks [list {rdc}] tag {RDC_CORRUPT_OBSERVED} \
    reason {D1: exact unconnected/no-clock sink. FIFO controller i_data is unused with MATCH_COUNT_WIDTH=0; RAM collision is unused with FORWARD_NEW_DATA=false. No sequential or output consumer. Count is coverage only.} \
    fields [dict create \
        {DestObject} [list {endpoint/writer/data/u_controller/i_data.data[63:0]}] \
        {DestObjectType} [list {unconnected}] \
        {RdcSourceResets:ResetName} [list {core_rst_n}] \
        {DestClockInfoList:DestClockInfo:ClockName} [list {'no-clock'}] \
    ] expected_count 1]
lappend findings [dict create \
    checks [list {rdc}] tag {RDC_CORRUPT_OBSERVED} \
    reason {D1: exact unconnected/no-clock sink. FIFO controller i_data is unused with MATCH_COUNT_WIDTH=0; RAM collision is unused with FORWARD_NEW_DATA=false. No sequential or output consumer. Count is coverage only.} \
    fields [dict create \
        {DestObject} [list {endpoint/writer/data/u_controller/i_data.data[7:0]}] \
        {DestObjectType} [list {unconnected}] \
        {RdcSourceResets:ResetName} [list {core_rst_n}] \
        {DestClockInfoList:DestClockInfo:ClockName} [list {'no-clock'}] \
    ] expected_count 1]
lappend findings [dict create \
    checks [list {rdc}] tag {RDC_CORRUPT_OBSERVED} \
    reason {D1: exact unconnected/no-clock sink. FIFO controller i_data is unused with MATCH_COUNT_WIDTH=0; RAM collision is unused with FORWARD_NEW_DATA=false. No sequential or output consumer. Count is coverage only.} \
    fields [dict create \
        {DestObject} [list endpoint/writer/data/u_controller/i_data.data\{ {[66:64]} {,[58:56]} {,[50:48]} {,[42:40]} {,[34:32]} {,[26:24]} {,[18:16]} {,[10:8]} {,[2:0]} \}] \
        {DestObjectType} [list {unconnected}] \
        {RdcSourceResets:ResetName} [list {core_rst_n}] \
        {DestClockInfoList:DestClockInfo:ClockName} [list {'no-clock'}] \
    ] expected_count 1]
lappend findings [dict create \
    checks [list {rdc}] tag {RDC_CORRUPT_OBSERVED} \
    reason {D1: exact unconnected/no-clock sink. FIFO controller i_data is unused with MATCH_COUNT_WIDTH=0; RAM collision is unused with FORWARD_NEW_DATA=false. No sequential or output consumer. Count is coverage only.} \
    fields [dict create \
        {DestObject} [list {endpoint/writer/data/u_controller/i_data.lane[2:0]}] \
        {DestObjectType} [list {unconnected}] \
        {RdcSourceResets:ResetName} [list {bus_rst_n}] \
        {DestClockInfoList:DestClockInfo:ClockName} [list {'no-clock'}] \
    ] expected_count 41]
lappend findings [dict create \
    checks [list {rdc}] tag {RDC_CORRUPT_OBSERVED} \
    reason {D1: exact unconnected/no-clock sink. FIFO controller i_data is unused with MATCH_COUNT_WIDTH=0; RAM collision is unused with FORWARD_NEW_DATA=false. No sequential or output consumer. Count is coverage only.} \
    fields [dict create \
        {DestObject} [list {endpoint/writer/data/u_controller/i_data.lane[2:0]}] \
        {DestObjectType} [list {unconnected}] \
        {RdcSourceResets:ResetName} [list {core_rst_n}] \
        {DestClockInfoList:DestClockInfo:ClockName} [list {'no-clock'}] \
    ] expected_count 54]
lappend findings [dict create \
    checks [list {rdc}] tag {RDC_CORRUPT_OBSERVED} \
    reason {D1: exact unconnected/no-clock sink. FIFO controller i_data is unused with MATCH_COUNT_WIDTH=0; RAM collision is unused with FORWARD_NEW_DATA=false. No sequential or output consumer. Count is coverage only.} \
    fields [dict create \
        {DestObject} [list {endpoint/writer/data/u_controller/i_data.length[2:0]}] \
        {DestObjectType} [list {unconnected}] \
        {RdcSourceResets:ResetName} [list {bus_rst_n}] \
        {DestClockInfoList:DestClockInfo:ClockName} [list {'no-clock'}] \
    ] expected_count 41]
lappend findings [dict create \
    checks [list {rdc}] tag {RDC_CORRUPT_OBSERVED} \
    reason {D1: exact unconnected/no-clock sink. FIFO controller i_data is unused with MATCH_COUNT_WIDTH=0; RAM collision is unused with FORWARD_NEW_DATA=false. No sequential or output consumer. Count is coverage only.} \
    fields [dict create \
        {DestObject} [list {endpoint/writer/data/u_controller/i_data.length[2:0]}] \
        {DestObjectType} [list {unconnected}] \
        {RdcSourceResets:ResetName} [list {core_rst_n}] \
        {DestClockInfoList:DestClockInfo:ClockName} [list {'no-clock'}] \
    ] expected_count 53]
lappend findings [dict create \
    checks [list {rdc}] tag {RDC_CORRUPT_OBSERVED} \
    reason {D1: exact unconnected/no-clock sink. FIFO controller i_data is unused with MATCH_COUNT_WIDTH=0; RAM collision is unused with FORWARD_NEW_DATA=false. No sequential or output consumer. Count is coverage only.} \
    fields [dict create \
        {DestObject} [list {endpoint/writer/data/u_controller/i_data.size[0]}] \
        {DestObjectType} [list {unconnected}] \
        {RdcSourceResets:ResetName} [list {core_rst_n}] \
        {DestClockInfoList:DestClockInfo:ClockName} [list {'no-clock'}] \
    ] expected_count 1]
lappend findings [dict create \
    checks [list {rdc}] tag {RDC_CORRUPT_OBSERVED} \
    reason {D1: exact unconnected/no-clock sink. FIFO controller i_data is unused with MATCH_COUNT_WIDTH=0; RAM collision is unused with FORWARD_NEW_DATA=false. No sequential or output consumer. Count is coverage only.} \
    fields [dict create \
        {DestObject} [list {endpoint/writer/data/u_controller/i_data.size[1:0]}] \
        {DestObjectType} [list {unconnected}] \
        {RdcSourceResets:ResetName} [list {bus_rst_n}] \
        {DestClockInfoList:DestClockInfo:ClockName} [list {'no-clock'}] \
    ] expected_count 41]
lappend findings [dict create \
    checks [list {rdc}] tag {RDC_CORRUPT_OBSERVED} \
    reason {D1: exact unconnected/no-clock sink. FIFO controller i_data is unused with MATCH_COUNT_WIDTH=0; RAM collision is unused with FORWARD_NEW_DATA=false. No sequential or output consumer. Count is coverage only.} \
    fields [dict create \
        {DestObject} [list {endpoint/writer/data/u_controller/i_data.size[1:0]}] \
        {DestObjectType} [list {unconnected}] \
        {RdcSourceResets:ResetName} [list {core_rst_n}] \
        {DestClockInfoList:DestClockInfo:ClockName} [list {'no-clock'}] \
    ] expected_count 53]
lappend findings [dict create \
    checks [list {rdc}] tag {RDC_CORRUPT_OBSERVED} \
    reason {D1: exact unconnected/no-clock sink. FIFO controller i_data is unused with MATCH_COUNT_WIDTH=0; RAM collision is unused with FORWARD_NEW_DATA=false. No sequential or output consumer. Count is coverage only.} \
    fields [dict create \
        {DestObject} [list {endpoint/writer/responses/u_controller/i_data[3:0]}] \
        {DestObjectType} [list {unconnected}] \
        {RdcSourceResets:ResetName} [list {bus_rst_n}] \
        {DestClockInfoList:DestClockInfo:ClockName} [list {'no-clock'}] \
    ] expected_count 41]
lappend findings [dict create \
    checks [list {rdc}] tag {RDC_CORRUPT_OBSERVED} \
    reason {D1: exact unconnected/no-clock sink. FIFO controller i_data is unused with MATCH_COUNT_WIDTH=0; RAM collision is unused with FORWARD_NEW_DATA=false. No sequential or output consumer. Count is coverage only.} \
    fields [dict create \
        {DestObject} [list {endpoint/writer/responses/u_controller/i_data[3:0]}] \
        {DestObjectType} [list {unconnected}] \
        {RdcSourceResets:ResetName} [list {core_rst_n}] \
        {DestClockInfoList:DestClockInfo:ClockName} [list {'no-clock'}] \
    ] expected_count 53]
lappend findings [dict create \
    checks [list {rdc}] tag {RDC_CORRUPT_OBSERVED} \
    reason {D1: exact unconnected/no-clock sink. FIFO controller i_data is unused with MATCH_COUNT_WIDTH=0; RAM collision is unused with FORWARD_NEW_DATA=false. No sequential or output consumer. Count is coverage only.} \
    fields [dict create \
        {DestObject} [list {engine/copy/buffers/collision}] \
        {DestObjectType} [list {unconnected}] \
        {RdcSourceResets:ResetName} [list {bus_rst_n}] \
        {DestClockInfoList:DestClockInfo:ClockName} [list {'no-clock'}] \
    ] expected_count 41]
lappend findings [dict create \
    checks [list {rdc}] tag {RDC_CORRUPT_OBSERVED} \
    reason {D1: exact unconnected/no-clock sink. FIFO controller i_data is unused with MATCH_COUNT_WIDTH=0; RAM collision is unused with FORWARD_NEW_DATA=false. No sequential or output consumer. Count is coverage only.} \
    fields [dict create \
        {DestObject} [list {engine/copy/buffers/collision}] \
        {DestObjectType} [list {unconnected}] \
        {RdcSourceResets:ResetName} [list {core_rst_n}] \
        {DestClockInfoList:DestClockInfo:ClockName} [list {'no-clock'}] \
    ] expected_count 53]
return [dict create identity $identity findings $findings]
