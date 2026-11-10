//! SV package consumers use 64-bit analyzed formulas without a UVM dependency.
#![cfg(feature = "export-sv")]
use hw_model_schema::{
    analysis::{AnalyzedSchema, Bindings},
    ast::{Definition, DescribeSchema, LayoutExpr, MapDefinition, MapPlacement, SchemaSet},
    export::{backends::system_verilog::SvRegisterBackend, generate},
    register_map, register_schema,
};
use std::fmt::Write as _;
register_schema! {pub register Status {width:4,endian:Little,access:ReadOnly,reset:Constant(1),writable_mask:0,fields:{ready:0..=0}}}
register_map! {pub map Ports(n:usize) {alignment:16,placements:[pub status:Status[n]]}}
struct Wide;
impl DescribeSchema for Wide {
    fn schema_definition() -> Definition {
        Definition::register_map::<Self>(
            "Wide",
            hw_model_schema::Documentation::EMPTY,
            MapDefinition::new(
                &["base"],
                4,
                None,
                vec![
                    MapPlacement::new(
                        "padding",
                        <Status as DescribeSchema>::schema_definition,
                        vec![],
                        LayoutExpr::Subtract(
                            Box::new(LayoutExpr::Parameter("base")),
                            Box::new(LayoutExpr::Constant(1_usize << 40)),
                        ),
                        None,
                        true,
                    ),
                    MapPlacement::new(
                        "status",
                        <Status as DescribeSchema>::schema_definition,
                        vec![],
                        LayoutExpr::Constant(1),
                        Some((1_usize << 40) + 8),
                        false,
                    ),
                ],
            ),
        )
    }
}
#[test]
fn native_fixture_matches_rust_geometry_and_preserves_large_addresses()
-> Result<(), Box<dyn std::error::Error>> {
    let mut set = SchemaSet::new();
    set.add::<Ports>();
    set.add::<Wide>();
    let schema = AnalyzedSchema::analyze(set, &Bindings::new())?;
    let mut bytes = Vec::new();
    generate(&schema, SvRegisterBackend::new(), &mut bytes)?;
    let text = String::from_utf8(bytes)?;
    assert!(text.contains("VALUE_READY_LSB"));
    assert!(text.contains("input logic [63:0] p_base"));
    assert!(text.contains("VALUE_WRITE = 1'b0"));
    let mut bench = String::from("module schema_register_metadata;\n initial begin\n");
    for count in [1_usize, 3, 257] {
        let expected = Ports::new(count)?.extent();
        writeln!(
            bench,
            " if (PortsLayout::extent(64'd{count}) != 64'd{expected} || PortsLayout::status_count(64'd{count}) != 64'd{count} || PortsLayout::status_stride(64'd{count}) != 4) $fatal(1, \"port geometry\");"
        )?;
    }
    bench.push_str(" if (WideLayout::status_offset(64'h10000000001) != 64'h10000000008 || WideLayout::extent(64'h10000000001) != 64'h1000000000c || WideLayout::padding_count(64'h10000000001) != 1) $fatal(1, \"wide address\");\n if (StatusLayout::VALUE_RESET != 1 || StatusLayout::VALUE_WRITE || !StatusLayout::VALUE_READ || StatusLayout::VALUE_READY_BITS != 1) $fatal(1, \"register metadata\");\n $display(\"SCHEMA_REGISTER_PASS\");\n $finish;\n end\nendmodule\n");
    if let Some(path) = std::env::var_os("SCHEMA_SV_REGISTER_FIXTURE") {
        std::fs::write(path, format!("{text}{bench}"))?;
    }
    Ok(())
}
#[test]
fn permission_projection_keeps_read_only_masks_and_holes() -> Result<(), Box<dyn std::error::Error>>
{
    let map = Ports::new(3)?
        .resolve()?
        .project_access(|_| Some(hw_model_schema::Access::ReadOnly))?;
    let schema = AnalyzedSchema::register_map("Projected", &map)?;
    let mut bytes = Vec::new();
    generate(&schema, SvRegisterBackend::new(), &mut bytes)?;
    let text = String::from_utf8(bytes)?;
    assert!(text.contains("STATUS_2_OFFSET = 64'd8"));
    assert!(text.contains("STATUS_2_WRITE = 1'b0"));
    Ok(())
}
#[test]
fn streaming_failure_is_propagated_from_layout_functions() -> Result<(), Box<dyn std::error::Error>>
{
    let mut set = SchemaSet::new();
    set.add::<Ports>();
    let schema = AnalyzedSchema::analyze(set, &Bindings::new())?;
    let mut backend = SvRegisterBackend::new();
    let roots = hw_model_schema::export::prepare(&schema, &mut backend)?;
    for entry in roots {
        let mut complete = Vec::new();
        entry.emit(&mut backend, &mut complete)?;
        for capacity in [0, complete.len() / 2, complete.len() - 1] {
            let mut storage = vec![0; capacity];
            let err = entry
                .emit(&mut backend, &mut storage.as_mut_slice())
                .unwrap_err();
            assert_eq!(err.kind(), std::io::ErrorKind::WriteZero);
        }
    }
    Ok(())
}
