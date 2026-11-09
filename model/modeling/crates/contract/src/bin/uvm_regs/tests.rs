use super::*;
use hw_model_schema::register_schema;

register_schema! {
    pub register Partial {
        width: 4, endian: Little, access: ReadWrite, reset: Zero(0), writable_mask: 3,
        fields: { data: 0..=3 },
    }
}
register_schema! {
    pub register Constant {
        width: 4, endian: Little, access: ReadWrite, reset: Constant(0xa5), writable_mask: 0xff,
        fields: { low: 0..=7, high: 8..=15 },
    }
}

register_schema! {
    pub register Ambiguous {
        width: 4, endian: Little, access: ReadOnly, reset: Zero(0), writable_mask: 0,
        fields: { code: 0..=1 => enum { variants: { MyValue = 0, Myvalue = 1 } } },
    }
}

#[test]
fn rejects_enum_names_that_collide_after_formatting() {
    let mut definitions = SchemaSet::new();
    definitions.add::<Ambiguous>();
    assert!(render(definitions).is_err());
}

#[test]
fn emits_hierarchical_register_model() {
    let text = generate().unwrap();
    assert!(text.contains("class DmaRegisters extends uvm_reg_block;"));
    assert!(text.contains("class Word extends uvm_reg;"));
    assert!(text.contains("ChannelBlock channels_block[];"));
    assert!(text.contains("admin_block.build(p_channel_count, bus_bytes)"));
    assert!(text.contains("default_map.add_submap(channels_block[i].default_map"));
}

#[test]
fn rejects_partial_field_write_permissions() {
    let mut definitions = SchemaSet::new();
    definitions.add::<Partial>();
    let error = render(definitions).unwrap_err().to_string();
    assert!(
        error.contains("Partial.data: partial writable field mask"),
        "{error}"
    );
}

#[test]
fn field_permissions_and_constant_reset_follow_schema() {
    let mut definitions = SchemaSet::new();
    definitions.add::<Constant>();
    let text = render(definitions).unwrap();
    assert!(text.contains("low_field.configure(this, 8, 0, \"RW\", 1, 64'ha5, 1, 0, 0)"));
    assert!(text.contains("high_field.configure(this, 8, 8, \"RO\", 1, 64'h0, 1, 0, 0)"));
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
fn rejects_permission_projected_maps_without_reusable_hierarchy() {
    let map = DmaRegisters::new(1).unwrap().resolve().unwrap();
    let restricted = map
        .project_access(|r| match r.access() {
            hw_model_schema::Access::ReadOnly | hw_model_schema::Access::ReadWrite => {
                Some(hw_model_schema::Access::ReadOnly)
            }
            hw_model_schema::Access::WriteOnly => None,
        })
        .unwrap();
    let schema = AnalyzedSchema::register_map("Restricted", &restricted).unwrap();
    let mut backend = backend::Uvm::default();
    let outputs = prepare_roots(&mut backend, register::roots(&schema)).unwrap();
    let error = outputs
        .into_iter()
        .next()
        .unwrap()
        .emit(&mut backend, &mut Vec::new())
        .unwrap_err();
    assert!(error.to_string().contains("permission-projected"));
}

#[test]
fn zero_geometry_parameter_and_reserved_names_are_supported() {
    use hw_model_schema::{
        Documentation, RegisterLayout,
        analysis::Layout,
        ast::{Definition, LayoutExpr, MapDefinition, MapPlacement},
    };
    use pdma_model_contract::register::Word;
    struct Parameterized;
    let mut definitions = SchemaSet::new();
    definitions.add_definition(Definition::register_map::<Parameterized>(
        "Parameterized",
        Documentation::EMPTY,
        MapDefinition::new(
            &["bus_bytes", "offset"],
            4,
            None,
            vec![
                MapPlacement::new(
                    "padding",
                    Word::schema_definition,
                    vec![],
                    LayoutExpr::Add(
                        Box::new(LayoutExpr::Parameter("offset")),
                        Box::new(LayoutExpr::Constant(1)),
                    ),
                    None,
                    true,
                ),
                MapPlacement::new(
                    "tail",
                    Word::schema_definition,
                    vec![],
                    LayoutExpr::Constant(1),
                    None,
                    false,
                ),
            ],
        ),
    ));
    let schema = AnalyzedSchema::analyze(definitions, &Bindings::new()).unwrap();
    let unit = schema.units().last().unwrap();
    let Layout::Map(map) = &unit.body else {
        panic!("map");
    };
    let geometry = map.plan.as_ref().unwrap().evaluate(&[0, 0]).unwrap();
    assert_eq!(geometry.placements.get(1).unwrap().offset, 4);
    let mut backend = backend::Uvm::default();
    let outputs = prepare_roots(&mut backend, register::roots(&schema)).unwrap();
    let mut bytes = Vec::new();
    for output in outputs {
        output.emit(&mut backend, &mut bytes).unwrap();
    }
    let text = String::from_utf8(bytes).unwrap();
    assert!(text.contains("input int unsigned p_bus_bytes, input int unsigned p_offset, input int unsigned bus_bytes = 4"));
    assert!(!text.contains("if (p_offset == 0)"));
    assert!(text.contains("if ((p_offset + 64'd1) == 0)"));
    assert!(text.contains("padding_reg = new[(p_offset + 64'd1)]"));
    if let Some(destination) = std::env::var_os("PDMA_UVM_PARAMETER_FIXTURE") {
        let fixture = format!(
            "package parameter_fixture;\n import uvm_pkg::*;\n `include \"uvm_macros.svh\"\n{text}endpackage\nmodule parameter_metadata;\n import uvm_pkg::*;\n import parameter_fixture::*;\n initial begin\n Parameterized model = new(\"model\");\n model.build(0,0,4);\n model.lock_model();\n if (model.padding_reg.size() != 1 || model.tail_reg.get_address() != 4) $fatal(1, \"zero parameter geometry\");\n $display(\"RAL_ZERO_PARAMETER_PASS\");\n $finish;\n end\nendmodule\n"
        );
        std::fs::write(destination, fixture).unwrap();
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

#[test]
fn writer_failure_propagates_from_output_rendering() {
    // Given an analyzed fixture and its complete output size.
    let mut definitions = SchemaSet::new();
    definitions.add::<DmaRegisters>();
    let schema = AnalyzedSchema::analyze(definitions, &Bindings::new()).unwrap();
    let mut backend = backend::Uvm::default();
    let outputs = prepare_roots(&mut backend, register::roots(&schema)).unwrap();
    for output in outputs {
        let mut complete = Vec::new();
        output.emit(&mut backend, &mut complete).unwrap();
        for capacity in [0, complete.len() / 2, complete.len() - 1] {
            let mut storage = vec![0; capacity];
            // When the actual output writer runs out of capacity.
            let error = output
                .emit(&mut backend, &mut storage.as_mut_slice())
                .unwrap_err();
            // Then the writer error reaches the caller, including late writes.
            assert_eq!(error.kind(), std::io::ErrorKind::WriteZero);
        }
    }
}

mod source_bits;
