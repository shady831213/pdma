//! Whole-node backends own recursive syntax; scopes are immutable per sibling call.
use hw_model_schema::analysis::{AnalyzedSchema, Bindings};
use hw_model_schema::analysis::{ConcretePlacement, Placement};
use hw_model_schema::export::generate;
use hw_model_schema::export::value::Backend as _;
use hw_model_schema::{
    ast::{self, Field, LayoutExpr, RecordDefinition, RegisterInfo, SchemaSet, record::Node},
    export::{
        Artifact, InMemoryArtifactSink,
        expression::{ExpressionSyntax, format_expression},
        record::{Backend, EmitContext},
        register, value,
    },
    record_schema, register_map, register_schema,
};
use std::{cell::RefCell, rc::Rc};
type TestResult = Result<(), Box<dyn std::error::Error>>;
record_schema! { pub record Packet { endian: Little, fields: {
    zeta: u32,
    alpha: bits(u8) { fields: { mode: 0..2 => enum { variants: { Off = 0, On = 1 } } } },
    beta: u8,
} } }
register_schema! { pub register Word { width:4, endian:Little, access:ReadOnly, reset:Zero(0), writable_mask:0, fields:{flag:0..1} } }
register_map! { pub map Pair { alignment:4, placements:[pub first:Word, pub second:Word] } }

struct Recording {
    events: Rc<RefCell<Vec<String>>>,
    fail: Option<&'static str>,
}
impl Recording {
    fn push(&self, value: impl Into<String>) {
        self.events.borrow_mut().push(value.into());
    }
}
impl Backend for Recording {
    fn record<'a>(
        &mut self,
        _: &'a RecordDefinition,
        ctx: EmitContext<'a>,
    ) -> Result<(), Self::Error> {
        self.push("record:start");
        let mut children: Vec<_> = ctx.children().collect();
        children.sort_by_key(|visit| match visit.node() {
            Node::Field(field) => field.name(),
            _ => "",
        });
        self.emit_all(children)?;
        self.push("record:end");
        Ok(())
    }
    fn field<'a>(&mut self, field: &'a Field, ctx: EmitContext<'a>) -> Result<(), Self::Error> {
        assert_eq!(ctx.field().map(Field::name), Some(field.name()));
        assert!(ctx.bit().is_none());
        self.push(format!("field:{}:start", field.name()));
        if self.fail == Some(field.name()) {
            return Err("field rejected");
        }
        self.emit_all(ctx.children())?;
        self.push(format!("field:{}:end", field.name()));
        Ok(())
    }
}
impl value::Backend for Recording {
    type Domain = hw_model_schema::export::record::Domain;
    type Error = &'static str;
    type Context<'a> = EmitContext<'a>;
    fn variant(
        &mut self,
        variant: &hw_model_schema::EnumVariantSpec,
        ctx: EmitContext<'_>,
    ) -> Result<(), Self::Error> {
        assert_eq!(ctx.field().map(Field::name), Some("alpha"));
        assert_eq!(ctx.bit(), Some("mode"));
        self.push(format!("variant:{}", variant.name()));
        Ok(())
    }
    fn finish(self) -> Result<Vec<Artifact>, Self::Error> {
        Ok(vec![Artifact {
            logical_name: "tree".into(),
            media_type: "text/plain".into(),
            contents: self.events.borrow().join("\n").into_bytes(),
        }])
    }
}

fn packet() -> Result<AnalyzedSchema, Box<dyn std::error::Error>> {
    let mut set = SchemaSet::new();
    set.add_record::<Packet>();
    Ok(AnalyzedSchema::from_graph(
        &set.resolve()?,
        &Bindings::new(),
    )?)
}

#[test]
fn complete_node_handlers_wrap_sorted_children_without_mutating_source() -> TestResult {
    // Given one declaration with source order different from target order.
    let analysis = packet()?;
    let events = Rc::new(RefCell::new(Vec::new()));
    let mut sink = InMemoryArtifactSink::new();
    // When a whole-record backend chooses a sorted borrowed iterator.
    generate(
        &analysis,
        Recording {
            events: events.clone(),
            fail: None,
        },
        &mut sink,
    )
    .map_err(|error| error.to_string())?;
    // Then local syntax surrounds recursion, and nested field/bit scope does not leak.
    assert_eq!(
        *events.borrow(),
        [
            "record:start",
            "field:alpha:start",
            "variant:Off",
            "variant:On",
            "field:alpha:end",
            "field:beta:start",
            "field:beta:end",
            "field:zeta:start",
            "field:zeta:end",
            "record:end"
        ]
    );
    let node = analysis.units().next().ok_or("missing record")?.node()?;
    let names: Vec<_> = node
        .children()
        .filter_map(|node| match node {
            ast::Node::Record(Node::Field(field)) => Some(field.name()),
            _ => None,
        })
        .collect();
    assert_eq!(names, ["zeta", "alpha", "beta"]);
    assert_eq!(sink.artifacts().len(), 1);
    Ok(())
}

#[test]
fn recursive_failure_stops_siblings_and_publishes_no_partial_artifacts() -> TestResult {
    // Given a backend rejecting the second sorted field.
    let analysis = packet()?;
    let events = Rc::new(RefCell::new(Vec::new()));
    let mut sink = InMemoryArtifactSink::new();
    // When generation enters the failing child.
    let result = generate(
        &analysis,
        Recording {
            events: events.clone(),
            fail: Some("beta"),
        },
        &mut sink,
    );
    // Then the exact error survives, later siblings/end syntax are skipped, and storage is untouched.
    assert_eq!(
        result,
        Err(hw_model_schema::export::ExportError::Render(
            "field rejected"
        ))
    );
    assert_eq!(
        *events.borrow(),
        [
            "record:start",
            "field:alpha:start",
            "variant:Off",
            "variant:On",
            "field:alpha:end",
            "field:beta:start"
        ]
    );
    assert!(sink.artifacts().is_empty());
    Ok(())
}

#[derive(Default)]
struct PlacementScopes(Vec<usize>);
impl register::Backend for PlacementScopes {
    fn concrete_placement<'a>(
        &mut self,
        _: ConcretePlacement<'a>,
        ctx: register::EmitContext<'a>,
    ) -> Result<(), Self::Error> {
        assert!(ctx.register().is_none());
        let Some(Placement::Concrete(ConcretePlacement::Register { view, .. })) = ctx.placement()
        else {
            return Err("expected concrete register");
        };
        self.0.push(view.offset);
        self.emit_all(ctx.children())
    }
    fn register<'a>(
        &mut self,
        register: &'a RegisterInfo,
        ctx: register::EmitContext<'a>,
    ) -> Result<(), Self::Error> {
        assert_eq!(
            ctx.register().map(RegisterInfo::name),
            Some(register.name())
        );
        assert!(ctx.bit().is_none());
        self.emit_all(ctx.children())
    }
}
impl value::Backend for PlacementScopes {
    type Domain = register::Domain;
    type Error = &'static str;
    type Context<'a> = register::EmitContext<'a>;
    fn finish(self) -> Result<Vec<Artifact>, Self::Error> {
        assert_eq!(self.0, [0, 4]);
        Ok(Vec::new())
    }
}
#[test]
fn sibling_placements_keep_independent_register_and_bit_scopes() -> TestResult {
    // Given a checked map with two adjacent register placements.
    let analysis = AnalyzedSchema::register_map("pair", &Pair::resolve()?)?;
    // When default recursion descends into both registers.
    generate(
        &analysis,
        PlacementScopes::default(),
        &mut InMemoryArtifactSink::new(),
    )
    .map_err(|error| error.to_string())?;
    // Then the backend assertions prove each sibling starts with a fresh register/bit scope.
    Ok(())
}

struct PrefixSyntax;
impl ExpressionSyntax for PrefixSyntax {
    fn binary(
        &mut self,
        op: hw_model_schema::ast::ExpressionOp,
        left: &str,
        right: &str,
    ) -> String {
        format!("({op:?} {left} {right})")
    }
    fn parameter(&mut self, name: &str) -> String {
        format!("Parameter({name:?})")
    }
}
#[test]
fn custom_syntax_uses_shared_recursive_expression_formatting() {
    // Given a source expression independent of any backend rendering protocol.
    let expression = LayoutExpr::Add(
        Box::new(LayoutExpr::Parameter("n")),
        Box::new(LayoutExpr::Constant(4)),
    );
    // When the shared expression fold applies custom operator and reference spelling.
    let text = format_expression(&expression, &mut PrefixSyntax).expect("complete formula");
    // Then syntax reflects both folded operands exactly.
    assert_eq!(text, "(Add Parameter(\"n\") 4)");
}
