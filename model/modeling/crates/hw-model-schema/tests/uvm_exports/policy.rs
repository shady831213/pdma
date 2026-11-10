use super::*;
use hw_model_schema::register_schema;
register_schema! {pub register Partial {width:4,endian:Little,access:ReadWrite,reset:Zero(0),writable_mask:3,fields:{data:0..=3}}}
register_schema! {pub register Constant {width:4,endian:Little,access:ReadWrite,reset:Constant(0xa5),writable_mask:0xff,fields:{low:0..=7,high:8..=15}}}
register_schema! {pub register Ambiguous {width:4,endian:Little,access:ReadOnly,reset:Zero(0),writable_mask:0,fields:{code:0..=1=>enum{variants:{MyValue=0,Myvalue=1}}}}}
register_schema! {pub register Bare {width:4,endian:Little,access:ReadWrite,reset:Zero(0),writable_mask:0xffff_ffff,fields:{}}}
#[test]
fn rejects_enum_names_that_collide_after_formatting() {
    let mut definitions = SchemaSet::new();
    definitions.add::<Ambiguous>();
    assert!(render(definitions).is_err());
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
fn rejects_permission_projected_maps_without_reusable_hierarchy() {
    let map = Root::new(1).unwrap().resolve().unwrap();
    let restricted = map
        .project_access(|r| match r.access() {
            hw_model_schema::Access::ReadOnly | hw_model_schema::Access::ReadWrite => {
                Some(hw_model_schema::Access::ReadOnly)
            }
            hw_model_schema::Access::WriteOnly => None,
        })
        .unwrap();
    let schema = AnalyzedSchema::register_map("Restricted", &restricted).unwrap();
    let mut backend = UvmRegisterBackend::new();
    let outputs = prepare_entries(&mut backend, register::entries(&schema)).unwrap();
    let error = outputs
        .into_iter()
        .next()
        .unwrap()
        .emit(&mut backend, &mut Vec::new())
        .unwrap_err();
    assert!(error.to_string().contains("permission-projected"));
}
#[test]
fn writer_failure_propagates_from_output_rendering() {
    // Given an analyzed fixture and its complete output size.
    let mut definitions = SchemaSet::new();
    definitions.add::<Root>();
    let schema = AnalyzedSchema::analyze(definitions, &Bindings::new()).unwrap();
    let mut backend = UvmRegisterBackend::new();
    let outputs = prepare_entries(&mut backend, register::entries(&schema)).unwrap();
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
