use super::*;
use hw_model_schema::ast::{Definition, DescribeSchema, LayoutExpr, MapDefinition, MapPlacement};
hw_model_schema::register_map! {pub map Shadow(n_reg:usize) {alignment:4,placements:[pub p_n:Word[n_reg]]}}
struct Addressed;
impl DescribeSchema for Addressed {
    fn schema_definition() -> Definition {
        Definition::register_map::<Self>(
            "Addressed",
            hw_model_schema::Documentation::EMPTY,
            MapDefinition::new(
                &["base"],
                4,
                None,
                vec![
                    MapPlacement::new(
                        "padding",
                        <Word as DescribeSchema>::schema_definition,
                        vec![],
                        LayoutExpr::Subtract(
                            Box::new(LayoutExpr::Parameter("base")),
                            Box::new(LayoutExpr::Constant(1_usize << 40)),
                        ),
                        None,
                        true,
                    ),
                    MapPlacement::new(
                        "word",
                        <Word as DescribeSchema>::schema_definition,
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
struct Coverage;
fn coverage_definition() -> Definition {
    use hw_model_schema::{
        BitsFieldDefinition, Endian, EnumDefinition, EnumVariantSpec, RegisterDefinition,
        RegisterFieldSpec,
    };
    const CODES: [EnumVariantSpec; 2] = [
        EnumVariantSpec::new("Zero", 0),
        EnumVariantSpec::new("NO_COVERAGE", 1),
    ];
    const FIELDS: [RegisterFieldSpec; 1] = [RegisterFieldSpec::place(
        0,
        "uvm",
        0,
        BitsFieldDefinition::new(1).with_enum(EnumDefinition::new(&CODES)),
    )];
    Definition::register::<Coverage, _>(
        RegisterDefinition::new("Coverage", 4, Endian::Little).with_fields(&FIELDS),
    )
}
#[test]
fn native_fixture_checks_64_bit_constructor_addresses() -> Result<(), Box<dyn std::error::Error>> {
    let mut set = SchemaSet::new();
    set.add::<Addressed>();
    set.add::<Shadow>();
    set.add_definition(coverage_definition());
    let text = render(set)?;
    assert!(text.contains("input longint unsigned p_base"));
    assert!(text.contains("uvm_pkg::UVM_NO_COVERAGE"));
    assert!(text.contains("this.p_n_reg = new[p_n_reg]"));
    if let Some(path) = std::env::var_os("SCHEMA_UVM_ADDRESS_FIXTURE") {
        let bench = "module schema_uvm_address;\n import uvm_pkg::*;\n import schema_uvm::*;\n initial begin\n Addressed model = new(\"model\");\n Shadow shadowed = new(\"shadowed\");\n Coverage coverage_reg = new(\"coverage_reg\");\n model.build(64'h10000000001,4);\n model.lock_model();\n if (model.word_reg.get_address() != 64'h10000000008 || model.padding_reg.size() != 1 || model.word_reg.mode_field.get_n_bits() != 2 || model.word_reg.mode_field.get_reset() != 2) $fatal(1, \"wide UVM address\");\n shadowed.build(2,4);\n shadowed.lock_model();\n if (shadowed.p_n_reg.size() != 2 || shadowed.p_n_reg[1].get_address() != 4) $fatal(1, \"parameter shadows member\");\n if (coverage_reg.has_coverage(uvm_pkg::UVM_CVR_REG_BITS)) $fatal(1, \"enum changed framework policy\");\n $display(\"SCHEMA_UVM_ADDRESS_PASS\");\n $finish;\n end\nendmodule\n";
        std::fs::write(
            path,
            format!(
                "package schema_uvm;\n import uvm_pkg::*;\n `include \"uvm_macros.svh\"\n{text}endpackage\n{bench}"
            ),
        )?;
    }
    Ok(())
}
