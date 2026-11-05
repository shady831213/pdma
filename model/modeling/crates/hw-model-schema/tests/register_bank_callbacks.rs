//! Targets consume normalized bank properties without resolving topology.
use hw_model_schema::traversal::register::{BankProperty, Context};
use hw_model_schema::{
    analysis::AnalyzedSchema,
    export::{self, Artifact, register, value},
    register_map, register_schema,
};
register_schema! { pub register Word { width:4, endian:Little, access:ReadOnly, reset:Zero(0), writable_mask:0, fields:{} } }
register_map! { pub map Leaf { alignment:4, placements:[pub word:Word] } }
register_map! { pub map Group { alignment:4, placements:[pub leaves:Leaf[1]] } }
register_map! { pub map Root { alignment:4, placements:[pub groups:Group[2]] } }
#[derive(Default)]
struct Banks(Vec<(usize, BankProperty)>, Vec<(usize, String, usize)>);
impl register::Backend for Banks {
    fn layout_property(
        &mut self,
        property: hw_model_schema::traversal::register::LayoutProperty<'_>,
        ctx: Context<'_>,
    ) -> Result<(), Self::Error> {
        self.1.push((
            ctx.bank().ok_or("missing bank")?.index(),
            property.name.into(),
            property
                .expression
                .evaluate(&[], &[])
                .map_err(|_| "invalid expression")?,
        ));
        Ok(())
    }
    fn bank_property(
        &mut self,
        property: BankProperty,
        ctx: Context<'_>,
    ) -> Result<(), Self::Error> {
        self.0
            .push((ctx.bank().ok_or("missing bank")?.index(), property));
        Ok(())
    }
}
impl value::Backend for Banks {
    type Domain = register::Domain;
    type Error = &'static str;
    type Context<'a> = Context<'a>;
    fn finish(self) -> Result<Vec<Artifact>, Self::Error> {
        use BankProperty as P;
        assert_eq!(
            self.0,
            vec![
                (0, P::Index(0)),
                (1, P::Index(1)),
                (1, P::ParentIndex(0)),
                (1, P::ParentInstance(0)),
                (2, P::Index(2)),
                (2, P::ParentIndex(0)),
                (2, P::ParentInstance(1))
            ]
        );
        assert_eq!(
            self.1,
            [
                (0, "offset", 0),
                (0, "count", 2),
                (0, "stride", 4),
                (1, "offset", 0),
                (1, "count", 1),
                (1, "stride", 4),
                (2, "offset", 4),
                (2, "count", 1),
                (2, "stride", 4)
            ]
            .map(|(bank, name, value)| (bank, name.into(), value))
        );
        Ok(Vec::new())
    }
}
#[test]
fn bank_properties_survive_empty_permission_projection() -> Result<(), Box<dyn std::error::Error>> {
    // Given nested banks whose register permissions hide every leaf.
    let map = Root::resolve()?.project_access(|_| None)?;
    let schema = AnalyzedSchema::register_map("root", &map)?;
    // When an ordinary backend consumes the shared traversal.
    export::generate(
        &schema,
        Banks::default(),
        &mut export::InMemoryArtifactSink::default(),
    )
    .map_err(|error| format!("{error:?}"))?;
    // Then finish checks stable bank identities, parents, singleton counts and bases.
    Ok(())
}
