//! Common preparation binds layouts and owns rendering order.
/// Caller-owned capture for generation tests.
pub mod output_support;
use hw_model_schema::analysis::{AnalyzedSchema, Bindings, Layout};
use hw_model_schema::export::register::Context as RegisterContext;
use hw_model_schema::{
    export::{register::Backend, value},
    register_map, register_schema,
};
use output_support::MemoryOutput;
register_schema! { pub register Word {width:4,endian:Little,access:ReadWrite,reset:Zero(0),writable_mask:0xffff_ffff,fields:{}} }
register_map! { pub map Block {alignment:16,placements:[pub word:Word]} }
register_map! { pub map Root(n:usize) {alignment:16,placements:[pub blocks:Block[n]]} }

#[test]
fn bound_and_symbolic_preparation_share_definition_identity()
-> Result<(), Box<dyn std::error::Error>> {
    // Given: one definition graph with an instance-bound root.
    let mut set = hw_model_schema::ast::SchemaSet::new();
    set.add::<Root>();
    let graph = set.resolve()?;
    let mut bindings = Bindings::new();
    bindings.bind::<Root>(&[3]);
    // When: the same graph is prepared with and without bindings.
    let symbolic = AnalyzedSchema::from_graph(&graph, &Bindings::new())?;
    let concrete = AnalyzedSchema::from_graph(&graph, &bindings)?;
    // Then: only evaluation differs; definition identity and dependencies remain.
    assert_eq!(
        symbolic.units().map(|d| d.id.clone()).collect::<Vec<_>>(),
        concrete.units().map(|d| d.id.clone()).collect::<Vec<_>>()
    );
    let root = concrete.units().find(|d| d.name == "Root").ok_or("root")?;
    let Layout::Map(map) = &root.body else {
        return Err("map".into());
    };
    assert_eq!(map.instance.as_ref().ok_or("bound instance")?.extent, 36);
    Ok(())
}

struct FailsAfterOne(usize);

register_map! { pub map Nested(n:usize) {alignment:16,placements:[pub roots:Root(n)[2]]} }
#[test]
fn projected_nested_bank_instances_use_their_own_ancestry() -> Result<(), Box<dyn std::error::Error>>
{
    // Given: two outer instances each contain two inner instances.
    let map = Nested::new(2)?
        .resolve()?
        .project_access(|_| Some(hw_model_schema::Access::ReadOnly))?;
    // When: the resolved permission view is prepared for any backend.
    let schema = AnalyzedSchema::register_map("view", &map)?;
    // Then: outer-bank groups follow outer indices and retain projected permissions.
    let Layout::Map(map) = &schema.units().next().ok_or("view")?.body else {
        return Err("map".into());
    };
    let bank = map
        .instance
        .as_ref()
        .ok_or("instance")?
        .banks
        .iter()
        .find(|b| b.name == "roots")
        .ok_or("bank")?;
    let offsets = bank
        .instances
        .iter()
        .map(|regs| regs.iter().map(|r| r.offset).collect::<Vec<_>>())
        .collect::<Vec<_>>();
    assert_eq!(offsets, [vec![0, 16], vec![32, 48]]);
    assert!(
        bank.instances
            .iter()
            .flatten()
            .all(|r| r.register.access() == hw_model_schema::Access::ReadOnly
                && r.register.writable_mask() == 0)
    );
    Ok(())
}

#[test]
fn named_instances_survive_preparation_with_independent_geometry()
-> Result<(), Box<dyn std::error::Error>> {
    // Given: two explicitly named instances share one reusable map definition.
    let mut set = hw_model_schema::ast::SchemaSet::new();
    set.add_map_instance::<Root>("small", &[1])?;
    set.add_map_instance::<Root>("large", &[3])?;
    // When: the complete selection is prepared for any renderer.
    let schema = AnalyzedSchema::from_graph(&set.resolve()?, &Bindings::new())?;
    // Then: both named instances retain their concrete extent.
    let instances = schema
        .units()
        .filter_map(|d| match (d.id.view_name(), &d.body) {
            (Some(name), Layout::Map(map)) => Some((name, map.instance.as_ref().map(|i| i.extent))),
            (None, _) | (Some(_), Layout::None | Layout::Record(_)) => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(instances, [("large", Some(36)), ("small", Some(4))]);
    Ok(())
}
impl Backend for FailsAfterOne {
    fn entry(
        &mut self,
        _: RegisterContext<'_>,
        _writer: &mut dyn std::io::Write,
    ) -> Result<(), Self::Error> {
        self.0 += 1;
        if self.0 == 2 {
            Err("render failure")
        } else {
            Ok(())
        }
    }
}
impl value::Backend for FailsAfterOne {
    type Domain = hw_model_schema::export::register::Domain;
    type Error = &'static str;
    type Context<'a> = RegisterContext<'a>;
}

#[test]
fn rendering_failure_preserves_completed_root_output() -> Result<(), Box<dyn std::error::Error>> {
    // Given: explicitly selected entries and a renderer failing on its second output.
    let mut set = hw_model_schema::ast::SchemaSet::new();
    set.add::<Word>();
    set.add::<Root>();
    let schema = AnalyzedSchema::from_graph(&set.resolve()?, &Bindings::new())?;
    let mut sink = MemoryOutput::new();
    // When: the common driver traverses the prepared definitions.
    let result = sink.generate(&schema, FailsAfterOne(0));
    // Then: the render error is preserved and only the first completed root remains.
    assert_eq!(
        result,
        Err(hw_model_schema::export::ExportError::Render(
            "render failure"
        ))
    );
    let [completed] = sink.outputs() else {
        return Err("expected first root only".into());
    };
    assert_eq!(
        completed.entry_name,
        schema.units().next().ok_or("first root")?.name
    );

    Ok(())
}

struct CountCallbacks<'a>(&'a std::cell::Cell<usize>);
impl Backend for CountCallbacks<'_> {
    fn entry(
        &mut self,
        _: RegisterContext<'_>,
        _writer: &mut dyn std::io::Write,
    ) -> Result<(), Self::Error> {
        self.0.set(self.0.get() + 1);
        Ok(())
    }
}
impl value::Backend for CountCallbacks<'_> {
    type Domain = hw_model_schema::export::register::Domain;
    type Error = core::convert::Infallible;
    type Context<'a> = RegisterContext<'a>;
}

hw_model_schema::record_schema! { pub record ZRecord { endian: Little, fields: { value: u32 } } }
#[test]
fn wrong_domain_is_rejected_before_any_renderer_callback() -> Result<(), Box<dyn std::error::Error>>
{
    // Given: a valid register sorts before a record in one mixed closure.
    let mut set = hw_model_schema::ast::SchemaSet::new();
    set.add::<Word>();
    set.add::<ZRecord>();
    let schema = AnalyzedSchema::from_graph(&set.resolve()?, &Bindings::new())?;
    assert_eq!(
        schema.units().next().ok_or("first definition")?.name,
        "Word"
    );
    let callbacks = std::cell::Cell::new(0);
    let mut sink = MemoryOutput::new();
    // When: register generation receives the incompatible closure.
    let result = sink.generate(&schema, CountCallbacks(&callbacks));
    // Then: preflight rejects the record without invoking the earlier valid callback.
    assert!(matches!(
        result,
        Err(hw_model_schema::export::ExportError::Analysis(
            hw_model_schema::analysis::AnalysisError::Domain {
                domain: "register",
                ..
            }
        ))
    ));
    assert_eq!(callbacks.get(), 0);
    assert!(sink.outputs().is_empty());
    Ok(())
}
