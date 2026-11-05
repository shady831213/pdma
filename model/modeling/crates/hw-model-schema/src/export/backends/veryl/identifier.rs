pub(super) fn is_identifier(value: &str) -> bool {
    value.starts_with(|c: char| c.is_ascii_alphabetic() || c == '_')
        && value.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
        && value != "_"
        && !value.starts_with("__")
        && !KEYWORDS
            .split_ascii_whitespace()
            .any(|keyword| keyword == value)
}

// Veryl grammar and analyzer reserved words, revision bacc44532af83058bdaab97dbe66ca1b89976be2.
const KEYWORDS: &str = concat!(
    "accept_on alias always always_comb always_ff always_latch and as assert assign assume ",
    "automatic bbool before begin bind bins binsof bit block break buf bufif0 bufif1 byte ",
    "case casex casez cell chandle checker class clock clock_negedge clock_posedge clocking ",
    "cmos config connect const constraint context continue converse cover covergroup ",
    "coverpoint cross deassign default defparam design disable dist do edge else embed end ",
    "endcase endchecker endclass endclocking endconfig endfunction endgenerate endgroup ",
    "endinterface endmodule endpackage endprimitive endprogram endproperty endsequence ",
    "endspecify endtable endtask enum event eventually expect export extends extern f32 f64 ",
    "false final first_match for force foreach forever fork forkjoin function gen generate ",
    "genvar global highz0 highz1 i16 i32 i64 i8 if if_reset iff ifnone ignore_bins ",
    "illegal_bins implements implies import in incdir include initial inout input inside inst ",
    "instance int integer interconnect interface intersect join join_any join_none large ",
    "lbool let liblist library local localparam logic longint lsb macromodule matches medium ",
    "mixin modport module msb nand negedge nettype new nexttime nmos nor noshowcancelled not ",
    "notif0 notif1 null or output outside p16 p32 p64 p8 package packed param parameter pmos ",
    "posedge primitive priority program property protected proto pub pull0 pull1 pulldown ",
    "pullup pulsestyle_ondetect pulsestyle_onevent pure rand randc randcase randsequence ",
    "rcmos real realtime ref reg reject_on release repeat reset reset_active reset_async_high ",
    "reset_async_low reset_inactive reset_sync_high reset_sync_low restrict return rev rnmos ",
    "rpmos rtran rtranif0 rtranif1 s_always s_eventually s_nexttime s_until s_until_with same ",
    "scalared sequence shortint shortreal showcancelled signed small soft solve specify ",
    "specparam static step string strong strong0 strong1 struct super supply0 supply1 switch ",
    "sync_accept_on sync_reject_on table tagged task this throughout time timeprecision ",
    "timeunit tran tranif0 tranif1 tri tri0 tri1 triand trior trireg true type typedef u16 ",
    "u32 u64 u8 union unique unique0 unsafe unsigned until until_with untyped use uwire var ",
    "vectored virtual void wait wait_order wand weak weak0 weak1 while wildcard wire with ",
    "within wor xnor xor ",
);
