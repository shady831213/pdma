use super::*;

#[test]
fn emits_hierarchical_register_model() {
    let text = generate().unwrap();
    assert!(text.contains("class DmaRegisters extends uvm_pkg::uvm_reg_block;"));
    assert!(text.contains("class Word extends uvm_pkg::uvm_reg;"));
    assert!(text.contains("ChannelBlock channels_block[];"));
    assert!(text.contains("admin_block.build(p_channel_count, bus_bytes)"));
    assert!(text.contains("default_map.add_submap(this.channels_block[i].default_map"));
}

#[test]
fn independent_resolved_schema_matches_sv_oracle() {
    for count in [1_usize, 3, 1020] {
        let channel_start = (20 + 4 * count).div_ceil(4096) * 4096;
        let map = DmaRegisters::new(count).unwrap().resolve().unwrap();
        assert_eq!(map.registers().len(), 5 + 18 * count);
        for i in 0..count {
            for (suffix, expected) in [
                ("command.base", 0),
                ("event.base", 24),
                ("control.irq.trap_ack", 76),
            ] {
                let path = format!("DmaRegisters.channels[{i}].{suffix}");
                let register = map.registers().iter().find(|r| r.path() == path).unwrap();
                assert_eq!(register.offset(), channel_start + 4096 * i + expected);
            }
        }
    }
}

#[test]
fn zero_actual_bank_counts_are_guarded() {
    let text = generate().unwrap();
    assert!(text.contains("if (p_channel_count == 0) `uvm_fatal(\"RAL_BANK_COUNT\""));
    assert!(!text.contains("RAL_PARAMETER"));
}

#[test]
fn enum_values_reach_the_generated_register_class() {
    let text = generate().unwrap();
    assert!(text.contains("localparam logic [63:0] CODE_INTERNAL = 64'd1;"));
    assert_eq!(text.matches("CODE_INTERNAL = 64'd1;").count(), 1);
}
