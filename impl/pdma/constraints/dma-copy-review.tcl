# Reviewed 2026-11-11; rationale and qualification: docs/rtl/cdc-review.md.
# These identities bind actual emitted RTL, source manifest, design profile,
# clock/reset constraints and native tool ID. Changes require a fresh review;
# never regenerate this record automatically to make a failing gate pass.
# R2 follows the user-confirmed RDC-STD-001 approach from pcie_modeling:
# https://github.com/shady831213/pcie_modeling/blob/master/docs/evidence/rtl-flow-toolchain-2026-09-21.md
# Approval is rebound to PDMA's exact instances below, not inherited wholesale.
# R1 is the separate Forward-slice analysis, not a Veryl stdlib exception.
set identity [dict create \
    rtl c727990cdb47cc8a64dd1a0514830bc404a5be4dc194da8e3bb32a1eadff9243 \
    design d37f43769804e93a9e64ffdaa94390428616d45848476b234c709b8936dabfb9 \
    profile 7f8bd80ff9f1239f0f4bc37c4e93c03fea99bc14713d78fdd3c824d60ca7ace8 \
    constraints afce778906a537807693f9321b589e1f388014d49d7d1b8c76f4b7c988462296 \
    common 51203b927c409b8dcec227cd78c1a36310395d4d43fd2908ef361ad2ee3b22a8 \
    analysis 4741e8d054e2dd71797f9613a2053f52fd447ae3647a995615a622183ca15dd6 \
    tool aec2fec538b2a0e09d2c9aa7cf17f060d8f3dc1436fa1384fe212e2faaf37761]
set findings {}
foreach {point tag qualifiers reason} {
    {csr/crossing/requests/g_dst_side.valid/D} CDC_COHERENCY_RECONV_SEQ {
        {csr/crossing/requests/u_s_to_d/g[0].rg/Q[1]}
        {csr/crossing/responses/u_d_to_s/g[0].rg/Q[1]}
    } {C1: request-valid persists until reply credit; independent handshake arrival order is allowed.}
    {endpoint/phase/D[0]} CDC_COHERENCY_RECONV_SEQ {
        {crossing/requests/u_s_to_d/g[0].rg/Q[1]}
        {crossing/responses/u_d_to_s/g[0].rg/Q[1]}
    } {C2: endpoint retains its slot through AXI and receipt acceptance; late credit only stalls.}
    {engine/phase/D[0]} CDC_COHERENCY_ASYNCSRCS_RECONV_SEQ {
        {csr/crossing/requests/u_s_to_d/g[0].rg/Q[1]}
        {csr/crossing/responses/u_d_to_s/g[0].rg/Q[1]}
        {crossing/requests/u_d_to_s/g[0].rg/Q[1]}
        {crossing/responses/u_s_to_d/g[0].rg/Q[1]}
    } {C3: reset closes issue, retains accepted debt and consumes its receipt before clear; no atomic multi-bit control encoding.}
} {
    lappend findings [dict create checks {cdc} tag $tag reason $reason fields [dict create \
        ConvergencePoint [list $point] ConvControlPathList:ConvergingControlPath:SyncOutput $qualifiers]]
}
foreach {reset destinations reason} {
    reg_rst_n {
        {csr/frontend/aw_storage/g_forward.data.address[31:0]}
        {csr/frontend/aw_storage/g_forward.data.identity[0]}
        {csr/frontend/w_storage/g_forward.data.data[31:0]}
        {csr/frontend/w_storage/g_forward.data.strobe[3:0]}
        {csr/frontend/ar_storage/g_forward.data.address[31:0]}
        {csr/frontend/ar_storage/g_forward.data.identity[0]}
    } {R1: coordinated cold reset clears slice validity and consumer ownership; fresh input overwrites payload before valid. RDC follows resetless storage to observable consumers.}
    core_rst_n {
        {crossing/requests/latched_data.address[63:0]}
        {crossing/requests/latched_data.length[2:0]}
        {crossing/requests/latched_data.data[511:0]}
        {crossing/requests/latched_data.size[1:0]}
        {crossing/requests/latched_data.selector[31:0]}
        {crossing/requests/latched_data.write}
    } {R2 / RDC-STD-001 pattern: PDMA-scoped resetless payload exception. Coordinated cold reset clears request/ACK and destination valid; fresh data precedes the new request. Software reset does not reset this transport; independent warm reset is excluded.}
} {
    foreach destination $destinations {
        lappend findings [dict create checks {cdc rdc} tag SETUP_RESET_DRIVING_NON_ASYNC_PIN \
            reason $reason fields [dict create ResetName [list $reset] NonResetInstance [list $destination]]]
    }
}
return [dict create identity $identity findings $findings]
