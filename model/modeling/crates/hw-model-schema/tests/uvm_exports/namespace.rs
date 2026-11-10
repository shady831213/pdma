//! UVM references share a target namespace with generated members and parameters.
use super::*;
use hw_model_schema::ast::DefinitionId;
register_map! {pub map Pair {alignment:4,placements:[pub first:Word,pub second:Word]}}

#[test]
fn generated_members_and_parameters_cannot_shadow_class_references()
-> Result<(), Box<dyn std::error::Error>> {
    let mut set = SchemaSet::new();
    set.add::<Root>();
    let schema = AnalyzedSchema::analyze(set, &Bindings::new())?;
    let word = schema
        .definition(DefinitionId::of::<Word>())
        .ok_or("word")?
        .id
        .clone();
    for alias in ["bus_bytes", "p_n", "words_reg"] {
        let error = generate(
            &schema,
            UvmRegisterBackend::new().with_name(word.clone(), alias),
            &mut Vec::new(),
        )
        .unwrap_err();
        assert!(error.to_string().contains(alias), "{error}");
    }
    Ok(())
}

#[test]
fn later_member_declarations_cannot_shadow_an_earlier_type_reference()
-> Result<(), Box<dyn std::error::Error>> {
    let mut set = SchemaSet::new();
    set.add::<Pair>();
    let schema = AnalyzedSchema::analyze(set, &Bindings::new())?;
    let word = schema
        .definition(DefinitionId::of::<Word>())
        .ok_or("word")?
        .id
        .clone();
    let error = generate(
        &schema,
        UvmRegisterBackend::new().with_name(word, "second_reg"),
        &mut Vec::new(),
    )
    .unwrap_err();
    assert!(error.to_string().contains("second_reg"), "{error}");
    Ok(())
}

#[test]
fn native_fixture_reuses_an_aliased_class_for_multiple_members()
-> Result<(), Box<dyn std::error::Error>> {
    let mut set = SchemaSet::new();
    set.add::<Pair>();
    set.add::<Root>();
    let schema = AnalyzedSchema::analyze(set, &Bindings::new())?;
    let word = schema
        .definition(DefinitionId::of::<Word>())
        .ok_or("word")?
        .id
        .clone();
    let mut bytes = Vec::new();
    generate(
        &schema,
        UvmRegisterBackend::new().with_name(word, "SafeWord"),
        &mut bytes,
    )?;
    if let Some(path) = std::env::var_os("SCHEMA_UVM_NAMESPACE_FIXTURE") {
        std::fs::write(
            path,
            format!(
                "package safe_alias;\n import uvm_pkg::*;\n `include \"uvm_macros.svh\"\n{}endpackage\nmodule schema_uvm_namespace;\n initial begin\n safe_alias::Pair pair_model = new(\"pair_model\");\n safe_alias::Root bank_model = new(\"bank_model\");\n pair_model.build(4);\n pair_model.lock_model();\n bank_model.build(2,4);\n bank_model.lock_model();\n if (pair_model.first_reg.get_address() != 0 || pair_model.second_reg.get_address() != 4 || bank_model.words_reg.size() != 2 || bank_model.words_reg[1].get_address() != 4) $fatal(1, \"alias reuse\");\n $display(\"SCHEMA_UVM_NAMESPACE_PASS\");\n $finish;\n end\nendmodule\n",
                String::from_utf8(bytes)?
            ),
        )?;
    }
    Ok(())
}
