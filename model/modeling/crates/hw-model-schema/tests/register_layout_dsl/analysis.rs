use super::{ControlRegister, LocatedChild, LocatedChildMap, NestedLocatedChildMap};
use hw_model_schema::{
    analysis::{AnalyzedSchema, Bindings, Layout},
    ast::SchemaSet,
    register_map,
};

register_map! {
    pub map OpaqueBank(n: usize) {
        alignment: 4,
        placements: [pub children: LocatedChild[n], pub control: ControlRegister],
    }
}

#[test]
fn custom_opaque_layouts_preserve_extent_without_inventing_registers()
-> Result<(), Box<dyn std::error::Error>> {
    // Given: fixed and nested maps whose custom child describes only its envelope.
    let mut set = SchemaSet::new();
    set.add::<LocatedChildMap>();
    set.add::<NestedLocatedChildMap>();
    let expected = [LocatedChildMap::EXTENT, NestedLocatedChildMap::EXTENT];
    // When: the same layouts cross the export-analysis boundary.
    let schema = AnalyzedSchema::analyze(set, &Bindings::new())?;
    let mut extents = Vec::new();
    for unit in schema.units() {
        if let Layout::Map(layout) = &unit.body {
            let instance = layout.instance.as_ref().ok_or("missing fixed instance")?;
            assert!(instance.registers.is_empty());
            extents.push(instance.extent);
        }
    }
    // Then: nested geometry survives, with no assumed layout inside the opaque child.
    assert_eq!(extents, expected);
    Ok(())
}

#[test]
fn bound_opaque_bank_preserves_following_register_location()
-> Result<(), Box<dyn std::error::Error>> {
    // Given: a bank of opaque children followed by a described register.
    let count = 3;
    let physical = OpaqueBank::new(count)?;
    let mut set = SchemaSet::new();
    set.add::<OpaqueBank>();
    let mut bindings = Bindings::new();
    bindings.bind::<OpaqueBank>(&[count]);
    // When: analysis binds the generic map.
    let schema = AnalyzedSchema::analyze(set, &bindings)?;
    let root = schema
        .definition(hw_model_schema::ast::DefinitionId::of::<OpaqueBank>())
        .ok_or("missing bank")?;
    let Layout::Map(layout) = &root.body else {
        return Err("expected map".into());
    };
    let instance = layout.instance.as_ref().ok_or("missing bound map")?;
    // Then: only the real register is exposed at its schema-derived offset.
    assert_eq!(instance.extent, physical.extent());
    assert_eq!(instance.registers.len(), 1);
    assert_eq!(
        instance.registers.first().ok_or("missing register")?.offset,
        physical.control().base()
    );
    let bank = instance.banks.first().ok_or("missing opaque bank")?;
    assert_eq!(bank.instances.len(), count);
    assert!(bank.instances.iter().all(Vec::is_empty));
    Ok(())
}
