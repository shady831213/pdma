use super::*;
#[test]
fn zero_geometry_parameter_and_reserved_names_are_supported() {
    use super::policy::Bare as Word;
    use hw_model_schema::{
        Documentation, RegisterLayout,
        analysis::Layout,
        ast::{Definition, LayoutExpr, MapDefinition, MapPlacement},
    };
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
    let mut backend = UvmRegisterBackend::new();
    let outputs = prepare_entries(&mut backend, register::entries(&schema)).unwrap();
    let mut bytes = Vec::new();
    for output in outputs {
        output.emit(&mut backend, &mut bytes).unwrap();
    }
    let text = String::from_utf8(bytes).unwrap();
    assert!(text.contains("input longint unsigned p_bus_bytes, input longint unsigned p_offset, input int unsigned bus_bytes = 4"));
    assert!(!text.contains("if (p_offset == 0)"));
    assert!(text.contains("if ((p_offset + 64'd1) == 0)"));
    assert!(text.contains("padding_reg = new[(p_offset + 64'd1)]"));
    if let Some(destination) = std::env::var_os("SCHEMA_UVM_PARAMETER_FIXTURE") {
        let fixture = format!(
            "package parameter_fixture;\n import uvm_pkg::*;\n `include \"uvm_macros.svh\"\n{text}endpackage\nmodule parameter_metadata;\n import uvm_pkg::*;\n import parameter_fixture::*;\n initial begin\n Parameterized model = new(\"model\");\n model.build(0,0,4);\n model.lock_model();\n if (model.padding_reg.size() != 1 || model.tail_reg.get_address() != 4) $fatal(1, \"zero parameter geometry\");\n $display(\"RAL_ZERO_PARAMETER_PASS\");\n $finish;\n end\nendmodule\n"
        );
        std::fs::write(destination, fixture).unwrap();
    }
}
