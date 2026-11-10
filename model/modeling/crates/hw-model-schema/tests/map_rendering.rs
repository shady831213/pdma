//! Each map has one default rendering path, regardless of parameter binding.
use hw_model_schema::{
    analysis::{AnalyzedSchema, Bindings},
    ast::SchemaSet,
    export::{
        register,
        value::{self, Emit},
        view,
    },
    register_map, register_schema,
};
register_schema! { pub register Word { width:4, endian:Little, access:ReadWrite,
reset:Zero(0), writable_mask:0xffff_ffff, fields:{} } }
register_map! { pub map Map(n:usize) { alignment:4, placements:[pub words:Word[n]] } }
struct Probe;
impl value::Backend for Probe {
    type Domain = register::Domain;
    type Error = std::io::Error;
    type Context<'a> = register::Context<'a>;
}
impl register::Backend for Probe {
    fn register_map(
        &mut self,
        _: view::Map<'_>,
        ctx: register::Context<'_>,
        w: &mut dyn std::io::Write,
    ) -> std::io::Result<()> {
        writeln!(w, "map")?;
        self.emit_all(ctx.children(), w)
    }
    fn layout_property(
        &mut self,
        property: register::LayoutProperty<'_>,
        _: register::Context<'_>,
        w: &mut dyn std::io::Write,
    ) -> std::io::Result<()> {
        writeln!(w, "{}", property.name())
    }
}
#[test]
fn symbolic_and_bound_maps_use_the_same_single_callback() -> Result<(), Box<dyn std::error::Error>>
{
    // Given the same parameterized declaration, with and without a binding.
    for bound in [false, true] {
        let mut set = SchemaSet::new();
        set.add::<Map>();
        let mut bindings = Bindings::new();
        if bound {
            bindings.bind::<Map>(&[3]);
        }
        let schema = AnalyzedSchema::analyze(set, &bindings)?;
        let mut bytes = Vec::new();
        // When a backend uses only default recursive traversal.
        Probe.emit_all(
            register::entries(&schema).filter(|v| v.name() == Some("Map")),
            &mut bytes,
        )?;
        let text = String::from_utf8(bytes)?;
        // Then every source map/property appears once in either form.
        assert_eq!(text.matches("map\n").count(), 1, "{text}");
        assert_eq!(text.matches("count\n").count(), 1, "{text}");
        assert_eq!(text.matches("extent\n").count(), 1, "{text}");
    }
    Ok(())
}

#[test]
fn explicit_bank_expansion_preserves_member_holes_indices_and_permissions()
-> Result<(), Box<dyn std::error::Error>> {
    use hw_model_schema::register_map::{RegisterMapSpec, RepeatedRegisterBank};
    use hw_model_schema::{Access, ast::RegisterLocation, traversal::register as walk};
    // Given a three-instance bank whose first member is hidden in every instance.
    const MEMBERS: [hw_model_schema::RegisterSpec<u8>; 2] = [
        hw_model_schema::RegisterSpec::place(0, "hidden", 0, Word::SPEC),
        hw_model_schema::RegisterSpec::place(1, "value", 4, Word::SPEC),
    ];
    const BANKS: [RepeatedRegisterBank<u8>; 1] =
        [RepeatedRegisterBank::new("words", 16, 3, 8, &MEMBERS)];
    let resolved = RegisterMapSpec::new(40, &[], &BANKS).resolve()?;
    let projected =
        resolved.project_access(|register| (register.id() == 1).then_some(Access::ReadOnly))?;
    let schema = AnalyzedSchema::register_map("visible", &projected)?;
    let root = walk::definitions(&schema).next().ok_or("map")?;
    let map = root.context().children().next().ok_or("map node")?;
    let bank = map
        .context()
        .children()
        .find(|v| matches!(v.node(), walk::Node::Bank(_)))
        .ok_or("bank")?;
    // When a consumer asks for concrete instances explicitly.
    let instances: Vec<_> = bank.context().instances().collect();
    assert_eq!(instances.len(), 3);
    // Then original instance indices and addresses survive, without write permission.
    for (index, instance) in instances.iter().enumerate() {
        let walk::Node::RegisterPlacement { view, location } = instance.node() else {
            return Err("placement".into());
        };
        assert_eq!(
            location,
            RegisterLocation::Instance {
                bank: 0,
                instance: index,
                member: 0
            }
        );
        assert_eq!(view.offset, 16 + index * 8 + 4);
        assert_eq!(view.register.access(), Access::ReadOnly);
        assert_eq!(view.register.writable_mask(), 0);
    }
    assert!(bank.context().children().all(|v| !matches!(
        v.node(),
        walk::Node::RegisterPlacement {
            location: RegisterLocation::Instance { .. },
            ..
        }
    )));
    Ok(())
}
