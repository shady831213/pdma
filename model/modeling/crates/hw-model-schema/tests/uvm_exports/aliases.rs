use super::*;
use hw_model_schema::ast::DefinitionId;
hw_model_schema::register_map! {pub map Parent(n:usize) {alignment:8,placements:[pub child:Root(n)]}}
#[test]
fn aliases_reach_both_class_declarations_and_member_initialization()
-> Result<(), Box<dyn std::error::Error>> {
    let mut set = SchemaSet::new();
    set.add::<Root>();
    let schema = AnalyzedSchema::analyze(set, &Bindings::new())?;
    let id = schema
        .definition(DefinitionId::of::<Word>())
        .ok_or("word")?
        .id
        .clone();
    let backend = UvmRegisterBackend::new()
        .with_name(id, "AliasedWord")
        .with_volatile(false)
        .with_mirror_comparison(true);
    let mut bytes = Vec::new();
    generate(&schema, backend, &mut bytes)?;
    let text = String::from_utf8(bytes)?;
    assert!(text.contains("class AliasedWord extends uvm_pkg::uvm_reg;"));
    assert!(text.contains("AliasedWord words_reg[];"));
    assert!(text.contains("AliasedWord::type_id::create"));
    assert!(text.contains("\"RW\", 0, 64'h2"));
    assert!(text.contains("set_compare(uvm_pkg::UVM_CHECK)"));
    Ok(())
}
#[test]
fn class_keyword_is_rejected_before_output() -> Result<(), Box<dyn std::error::Error>> {
    let mut set = SchemaSet::new();
    set.add::<Root>();
    let schema = AnalyzedSchema::analyze(set, &Bindings::new())?;
    let id = schema
        .definition(DefinitionId::of::<Word>())
        .ok_or("word")?
        .id
        .clone();
    let mut bytes = Vec::new();
    assert!(
        generate(
            &schema,
            UvmRegisterBackend::new().with_name(id, "module"),
            &mut bytes
        )
        .is_err()
    );
    assert!(bytes.is_empty());
    Ok(())
}

#[test]
fn uvm_macro_namespace_is_reserved_for_framework_types() -> Result<(), Box<dyn std::error::Error>> {
    let mut set = SchemaSet::new();
    set.add::<Word>();
    let schema = AnalyzedSchema::analyze(set, &Bindings::new())?;
    let id = schema
        .definition(DefinitionId::of::<Word>())
        .ok_or("word")?
        .id
        .clone();
    let mut bytes = Vec::new();
    assert!(
        generate(
            &schema,
            UvmRegisterBackend::new().with_name(id, "uvm_object_registry"),
            &mut bytes
        )
        .is_err()
    );
    assert!(bytes.is_empty());
    Ok(())
}
#[test]
fn standalone_bindings_preserve_constructor_arity_and_referenced_bindings_are_rejected()
-> Result<(), Box<dyn std::error::Error>> {
    let mut set = SchemaSet::new();
    set.add::<Root>();
    let mut bindings = Bindings::new();
    bindings.bind::<Root>(&[3]);
    let schema = AnalyzedSchema::analyze(set, &bindings)?;
    let mut bytes = Vec::new();
    generate(&schema, UvmRegisterBackend::new(), &mut bytes)?;
    let text = String::from_utf8(bytes)?;
    assert!(!text.contains("p_n"));
    assert!(text.contains("words_reg = new[64'd3]"));
    let mut mixed = SchemaSet::new();
    mixed.add::<Parent>();
    mixed.add::<Root>();
    assert!(matches!(
        AnalyzedSchema::analyze(mixed, &bindings),
        Err(hw_model_schema::analysis::AnalysisError::ReferencedBinding(
            _
        ))
    ));
    Ok(())
}
