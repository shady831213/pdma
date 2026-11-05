//! Whole-node backends own recursive syntax; scopes are immutable per sibling call.
/// Caller-owned capture for generation tests.
pub mod output_support;
use hw_model_schema::analysis::{AnalyzedSchema, Bindings};
use hw_model_schema::export::value::Backend as _;
use hw_model_schema::traversal::record::Context;
use hw_model_schema::{
    ast::{self, Field, LayoutExpr, RecordDefinition, RegisterInfo, SchemaSet, record::Node},
    export::{
        expression::{ExpressionSyntax, format_expression},
        record::Backend,
        register, value,
    },
    record_schema, register_map, register_schema,
};
use output_support::MemoryOutput;
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
        ctx: Context<'a>,
        writer: &mut dyn std::io::Write,
    ) -> Result<(), Self::Error> {
        self.push("record:start");
        let mut children: Vec<_> = ctx.children().collect();
        children.sort_by_key(|visit| match visit.node() {
            Node::Field(field) => field.name(),
            _ => "",
        });
        self.emit_all(children, writer)?;
        self.push("record:end");
        Ok(())
    }
    fn field<'a>(
        &mut self,
        field: &'a Field,
        ctx: Context<'a>,
        writer: &mut dyn std::io::Write,
    ) -> Result<(), Self::Error> {
        assert_eq!(ctx.field().map(Field::name), Some(field.name()));
        assert!(ctx.bit().is_none());
        self.push(format!("field:{}:start", field.name()));
        if self.fail == Some(field.name()) {
            return Err("field rejected");
        }
        self.emit_all(ctx.children(), writer)?;
        self.push(format!("field:{}:end", field.name()));
        Ok(())
    }
}
impl value::Backend for Recording {
    type Domain = hw_model_schema::export::record::Domain;
    type Error = &'static str;
    type Context<'a> = Context<'a>;
    fn variant(
        &mut self,
        variant: &hw_model_schema::EnumVariantSpec,
        ctx: Context<'_>,
        _writer: &mut dyn std::io::Write,
    ) -> Result<(), Self::Error> {
        assert_eq!(ctx.field().map(Field::name), Some("alpha"));
        assert_eq!(ctx.bit(), Some("mode"));
        self.push(format!("variant:{}", variant.name()));
        Ok(())
    }
    fn metadata(
        &mut self,
        ctx: &Self::Context<'_>,
    ) -> Result<hw_model_schema::export::OutputMetadata, Self::Error> {
        Ok(hw_model_schema::export::OutputMetadata {
            logical_name: ctx.unit().name.clone(),
            media_type: "text/plain".into(),
        })
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
    let mut sink = MemoryOutput::new();
    // When a whole-record backend chooses a sorted borrowed iterator.
    sink.generate(
        &analysis,
        Recording {
            events: events.clone(),
            fail: None,
        },
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
    assert_eq!(sink.outputs().len(), 1);
    Ok(())
}

#[test]
fn recursive_failure_stops_siblings_and_publishes_no_partial_artifacts() -> TestResult {
    // Given a backend rejecting the second sorted field.
    let analysis = packet()?;
    let events = Rc::new(RefCell::new(Vec::new()));
    let mut sink = MemoryOutput::new();
    // When generation enters the failing child.
    let result = sink.generate(
        &analysis,
        Recording {
            events: events.clone(),
            fail: Some("beta"),
        },
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
    assert!(sink.outputs().is_empty());
    Ok(())
}

#[derive(Default)]
struct PlacementScopes(Vec<usize>);
impl register::Backend for PlacementScopes {
    fn root(
        &mut self,
        ctx: Self::Context<'_>,
        writer: &mut dyn std::io::Write,
    ) -> Result<(), Self::Error> {
        self.emit(ctx.current(), writer)?;

        assert_eq!(self.0, [0, 4]);
        Ok(())
    }

    fn register_placement<'a>(
        &mut self,
        view: &'a hw_model_schema::analysis::RegisterView,
        ctx: hw_model_schema::traversal::register::Context<'a>,
        writer: &mut dyn std::io::Write,
    ) -> Result<(), Self::Error> {
        assert!(ctx.register().is_none());
        self.0.push(view.offset);
        self.emit_all(ctx.children(), writer)
    }
    fn register<'a>(
        &mut self,
        register: &'a RegisterInfo,
        ctx: hw_model_schema::traversal::register::Context<'a>,
        writer: &mut dyn std::io::Write,
    ) -> Result<(), Self::Error> {
        assert_eq!(
            ctx.register().map(RegisterInfo::name),
            Some(register.name())
        );
        assert!(ctx.bit().is_none());
        self.emit_all(ctx.children(), writer)
    }
}
impl value::Backend for PlacementScopes {
    type Domain = register::Domain;
    type Error = &'static str;
    type Context<'a> = hw_model_schema::traversal::register::Context<'a>;
    fn metadata(
        &mut self,
        ctx: &Self::Context<'_>,
    ) -> Result<hw_model_schema::export::OutputMetadata, Self::Error> {
        Ok(hw_model_schema::export::OutputMetadata {
            logical_name: ctx.unit().name.clone(),
            media_type: "text/plain".into(),
        })
    }
}
#[test]
fn sibling_placements_keep_independent_register_and_bit_scopes() -> TestResult {
    // Given a checked map with two adjacent register placements.
    let analysis = AnalyzedSchema::register_map("pair", &Pair::resolve()?)?;
    // When default recursion descends into both registers.
    MemoryOutput::new()
        .generate(&analysis, PlacementScopes::default())
        .map_err(|error| error.to_string())?;
    // Then the backend assertions prove each sibling starts with a fresh register/bit scope.
    Ok(())
}

mod expression_syntax_probe;
