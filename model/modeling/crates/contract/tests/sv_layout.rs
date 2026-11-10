//! Plain SV register packages support consumers that do not use Veryl or UVM.
#![cfg(feature = "export-sv")]
use std::process::Command;
#[test]
fn dma_sv_layout_exposes_symbolic_and_fixed_register_metadata() {
    let output = Command::new(env!("CARGO_BIN_EXE_pdma-sv-layout"))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let text = String::from_utf8(output.stdout).unwrap();
    for item in [
        "package DmaRegistersLayout;",
        "channels_offset(input logic [63:0] p_channel_count)",
        "CONTROL_RECOVERY_RESUME_OFFSET = 64'd60;",
        "CONTROL_IRQ_MASK_WRITABLE = 64'h0000000000000003;",
    ] {
        assert!(text.contains(item), "missing {item}");
    }
    assert!(!text.contains("uvm_"));
}
