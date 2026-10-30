//! Common preparation binds layouts and owns rendering order.
use hw_model_schema::{
    export::{
        Artifact, Bindings, InMemoryArtifactSink, PreparedBody, PreparedSchema, Renderer, generate,
    },
    register_map, register_schema,
};
register_schema! { pub register Word {width:4,endian:Little,access:ReadWrite,reset:Zero(0),writable_mask:0xffff_ffff,fields:{}} }
register_map! { pub map Block {alignment:16,placements:[pub word:Word]} }
register_map! { pub map Root(n:usize) {alignment:16,placements:[pub blocks:Block[n]]} }

#[test]
fn bound_and_symbolic_preparation_share_definition_identity()
-> Result<(), Box<dyn std::error::Error>> {
    // Given: one definition graph with an instance-bound root.
    let mut set = hw_model_schema::graph::SchemaSet::new();
    set.add::<Root>();
    let graph = set.resolve()?;
    let mut bindings = Bindings::new();
    bindings.bind::<Root>(&[3]);
    // When: the same graph is prepared with and without bindings.
    let symbolic = PreparedSchema::from_graph(&graph, &Bindings::new())?;
    let concrete = PreparedSchema::from_graph(&graph, &bindings)?;
    // Then: only evaluation differs; definition identity and dependencies remain.
    assert_eq!(
        symbolic
            .definitions()
            .map(|d| d.id.clone())
            .collect::<Vec<_>>(),
        concrete
            .definitions()
            .map(|d| d.id.clone())
            .collect::<Vec<_>>()
    );
    let root = concrete
        .definitions()
        .find(|d| d.name == "Root")
        .ok_or("root")?;
    let PreparedBody::Map(map) = &root.body else {
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
    let schema = PreparedSchema::register_map("view", &map);
    // Then: outer-bank groups follow outer indices and retain projected permissions.
    let PreparedBody::Map(map) = &schema.definitions().next().ok_or("view")?.body else {
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
    let mut set = hw_model_schema::graph::SchemaSet::new();
    set.add_map_instance::<Root>("small", &[1])?;
    set.add_map_instance::<Root>("large", &[3])?;
    // When: the complete selection is prepared for any renderer.
    let schema = PreparedSchema::from_graph(&set.resolve()?, &Bindings::new())?;
    // Then: both named instances retain their concrete extent.
    let instances = schema
        .definitions()
        .filter_map(|d| match (&d.id, &d.body) {
            (hw_model_schema::export::ExportId::View(name), PreparedBody::Map(map)) => {
                Some((name.as_str(), map.instance.as_ref().map(|i| i.extent)))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(instances, [("large", Some(36)), ("small", Some(4))]);
    Ok(())
}
impl Renderer for FailsAfterOne {
    type Error = &'static str;
    fn begin_definition(
        &mut self,
        _: hw_model_schema::export::DefinitionHeader<'_>,
    ) -> Result<(), Self::Error> {
        self.0 += 1;
        if self.0 == 2 {
            Err("render failure")
        } else {
            Ok(())
        }
    }
    fn finish(self) -> Result<Vec<Artifact>, Self::Error> {
        Ok(Vec::new())
    }
}
#[test]
fn rendering_failure_never_reaches_the_sink() -> Result<(), Box<dyn std::error::Error>> {
    // Given: multiple definitions and a renderer failing on the second definition.
    let mut set = hw_model_schema::graph::SchemaSet::new();
    set.add::<Root>();
    let schema = PreparedSchema::from_graph(&set.resolve()?, &Bindings::new())?;
    let mut sink = InMemoryArtifactSink::new();
    // When: the common driver traverses the prepared definitions.
    let result = generate(&schema, FailsAfterOne(0), &mut sink);
    // Then: the render error is preserved and nothing is emitted.
    assert_eq!(
        result,
        Err(hw_model_schema::export::ExportError::Render(
            "render failure"
        ))
    );
    assert!(sink.artifacts().is_empty());
    Ok(())
}
