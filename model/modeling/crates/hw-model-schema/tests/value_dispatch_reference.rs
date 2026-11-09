//! Register placements expose references as independently dispatched occurrences.
use hw_model_schema::{
    analysis::{AnalyzedSchema, Bindings},
    ast::SchemaSet,
    export::{
        Kind, OutputMetadata, register,
        value::{self, Emit as _},
        view,
    },
    register_map, register_schema,
};
register_schema! { pub register Word { width: 4, endian: Little, access: ReadOnly, reset: Zero(0), writable_mask: 0, fields: {} } }
register_map! { pub map Child(m: usize) { alignment: 4, placements: [pub words: Word[m]] } }
register_map! { pub map Parent(n: usize) { alignment: 4, placements: [pub child: Child(n)] } }

#[derive(Default)]
struct Probe {
    placements: Vec<String>,
    targets: Vec<String>,
    expand: bool,
}
impl register::Backend for Probe {
    fn argument<'a>(
        &mut self,
        _: hw_model_schema::export::view::Argument<'a>,
        ctx: register::Context<'a>,
        writer: &mut dyn std::io::Write,
    ) -> Result<(), Self::Error> {
        self.emit_all(ctx.children(), writer)
    }
    fn placement<'a>(
        &mut self,
        placement: view::Placement<'a>,
        ctx: register::Context<'a>,
        writer: &mut dyn std::io::Write,
    ) -> Result<(), Self::Error> {
        for visit in ctx
            .children()
            .filter(|visit| visit.kind() == Kind::Reference)
        {
            self.placements.push(placement.name().into());
            self.emit(visit, writer)?;
        }
        Ok(())
    }
    fn reference<'a>(
        &mut self,
        _: view::Reference<'a>,
        target: view::Target<'a>,
        visit: hw_model_schema::export::Visit<register::Context<'a>>,
        ctx: register::Context<'a>,
        writer: &mut dyn std::io::Write,
    ) -> Result<(), Self::Error> {
        self.targets.push(target.name.into());
        if !self.expand {
            return Ok(());
        }
        writeln!(
            writer,
            "reference:{}:{}",
            target.name,
            ctx.parameters().join(",")
        )?;
        self.emit_all(ctx.arguments(), writer)?;
        self.emit(visit.clone(), writer)?;
        self.emit(visit, writer)
    }
    fn map_layout<'a>(
        &mut self,
        _: view::MapLayout<'a>,
        ctx: register::Context<'a>,
        writer: &mut dyn std::io::Write,
    ) -> Result<(), Self::Error> {
        if ctx.root_name() == "Child" {
            return writeln!(
                writer,
                "definition:{}:{}",
                ctx.root_name(),
                ctx.parameters().join(",")
            );
        }
        self.emit_all(ctx.children(), writer)
    }
    fn layout_property<'a>(
        &mut self,
        _: view::LayoutProperty<'a>,
        _: register::Context<'a>,
        _: &mut dyn std::io::Write,
    ) -> Result<(), Self::Error> {
        Ok(())
    }
}
impl value::Backend for Probe {
    type Domain = register::Domain;
    type Error = std::io::Error;
    type Context<'a> = register::Context<'a>;

    fn expression<'a>(
        &mut self,
        expression: view::Expression<'a>,
        ctx: register::Context<'a>,
        writer: &mut dyn std::io::Write,
    ) -> Result<(), Self::Error> {
        let number = expression
            .evaluate(ctx.parameters(), &[3])
            .map_err(std::io::Error::other)?;
        writeln!(writer, "argument:{number}")
    }
}
impl hw_model_schema::export::OutputBackend for Probe {
    fn metadata(&mut self, ctx: &Self::Context<'_>) -> Result<OutputMetadata, Self::Error> {
        Ok(OutputMetadata {
            logical_name: ctx.root_name().into(),
            media_type: "text/plain".into(),
        })
    }
}

#[test]
fn placement_reference_has_an_independent_bound_visit() {
    // Given: a parameterized child map placed in its parent.
    let mut set = SchemaSet::new();
    set.add::<Parent>();
    let schema = AnalyzedSchema::analyze(set, &Bindings::new()).unwrap();
    let mut probe = Probe::default();
    // When: a target chooses only reference children of placements.
    probe
        .emit_all(
            register::roots(&schema).filter(|v| v.name() == Some("Parent")),
            &mut std::io::sink(),
        )
        .unwrap();
    // Then: the child reference is available to shared dispatch.
    assert_eq!(probe.placements, ["child"]);
    assert_eq!(probe.targets, ["Child"]);
}

#[test]
fn reference_target_and_constructor_arguments_keep_distinct_parameter_scopes() {
    use hw_model_schema::{
        Documentation,
        ast::{Definition, DescribeSchema, LayoutExpr, MapDefinition, MapPlacement},
    };
    // Given: formal child parameter m bound to the parent expression n + 1.
    let argument = LayoutExpr::Add(
        Box::new(LayoutExpr::Parameter("n")),
        Box::new(LayoutExpr::Constant(1)),
    );
    let placement = MapPlacement::new(
        "child",
        Child::schema_definition,
        vec![argument],
        LayoutExpr::Constant(1),
        None,
        false,
    );
    let mut set = SchemaSet::new();
    set.add_definition(Definition::register_map::<Parent>(
        "Parent",
        Documentation::EMPTY,
        MapDefinition::new(&["n"], 4, None, vec![placement]),
    ));
    let schema = AnalyzedSchema::analyze(set, &Bindings::new()).unwrap();
    let mut probe = Probe {
        expand: true,
        ..Probe::default()
    };
    let mut output = Vec::new();
    // When: the reference callback evaluates its argument and repeats target expansion.
    probe
        .emit_all(
            register::roots(&schema).filter(|v| v.name() == Some("Parent")),
            &mut output,
        )
        .unwrap();
    // Then: n=3 evaluates the use-site argument to 4 while both target visits retain m.
    assert_eq!(
        output,
        b"reference:Child:n\nargument:4\ndefinition:Child:m\ndefinition:Child:m\n"
    );
}
