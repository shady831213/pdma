//! Whole-node backends own recursive syntax; scopes are immutable per sibling call.
use hw_model_schema::{
    ast::{
        Field, MapView, Node, PlacementView, RecordDefinition, RegisterInfo, SchemaSet,
        analysis::{
            Analysis, Bindings,
            program::{Expression, Symbol},
        },
    },
    export::{
        Artifact, Backend, InMemoryArtifactSink, Scope, emit_all,
        expression::{ExpressionSyntax, format_expression},
        generate,
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
    type Error = &'static str;
    fn record(&mut self, record: &RecordDefinition, scope: Scope<'_>) -> Result<(), Self::Error> {
        self.push("record:start");
        let mut children: Vec<_> = Node::Record(record).children().collect();
        children.sort_by_key(|node| match node {
            Node::Field(field) => field.name(),
            _ => "",
        });
        emit_all(children, scope, self)?;
        self.push("record:end");
        Ok(())
    }
    fn field(&mut self, field: &Field, scope: Scope<'_>) -> Result<(), Self::Error> {
        assert_eq!(scope.field.map(Field::name), Some(field.name()));
        assert!(scope.bit.is_none());
        assert!(scope.placement.is_none());
        self.push(format!("field:{}:start", field.name()));
        if self.fail == Some(field.name()) {
            return Err("field rejected");
        }
        emit_all(Node::Field(field).children(), scope, self)?;
        self.push(format!("field:{}:end", field.name()));
        Ok(())
    }
    fn variant(
        &mut self,
        variant: &hw_model_schema::EnumVariantSpec,
        scope: Scope<'_>,
    ) -> Result<(), Self::Error> {
        assert_eq!(scope.field.map(Field::name), Some("alpha"));
        assert_eq!(scope.bit, Some("mode"));
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

fn packet() -> Result<Analysis, Box<dyn std::error::Error>> {
    let mut set = SchemaSet::new();
    set.add_record::<Packet>();
    Ok(Analysis::from_graph(&set.resolve()?, &Bindings::new())?)
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
    let node = analysis
        .definitions()
        .next()
        .ok_or("missing record")?
        .node()?;
    let names: Vec<_> = node
        .children()
        .filter_map(|node| match node {
            Node::Field(field) => Some(field.name()),
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
impl Backend for PlacementScopes {
    type Error = &'static str;
    fn register_map(&mut self, map: MapView<'_>, scope: Scope<'_>) -> Result<(), Self::Error> {
        emit_all(map.children(), scope, self)
    }
    fn placement(
        &mut self,
        placement: PlacementView<'_>,
        scope: Scope<'_>,
    ) -> Result<(), Self::Error> {
        assert!(scope.register.is_none());
        assert!(scope.field.is_none());
        let Some(PlacementView::Register { view, .. }) = scope.placement else {
            return Err("expected concrete register");
        };
        self.0.push(view.offset);
        emit_all(placement.children(), scope, self)
    }
    fn register(&mut self, register: &RegisterInfo, scope: Scope<'_>) -> Result<(), Self::Error> {
        assert_eq!(
            scope.register.map(RegisterInfo::name),
            Some(register.name())
        );
        assert!(scope.bit.is_none());
        emit_all(Node::Register(register).children(), scope, self)
    }
    fn finish(self) -> Result<Vec<Artifact>, Self::Error> {
        assert_eq!(self.0, [0, 4]);
        Ok(Vec::new())
    }
}
#[test]
fn sibling_placements_keep_independent_register_and_bit_scopes() -> TestResult {
    // Given a checked map with two adjacent register placements.
    let analysis = Analysis::register_map("pair", &Pair::resolve()?);
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
    fn reference(&mut self, reference: Symbol) -> String {
        format!("{reference:?}")
    }
}
#[test]
fn custom_syntax_uses_shared_recursive_expression_formatting() {
    // Given a source expression independent of any backend rendering protocol.
    let expression = Expression::Binary {
        op: hw_model_schema::ast::ExpressionOp::Add,
        left: Box::new(Expression::Reference(Symbol::Parameter("n"))),
        right: Box::new(Expression::Literal(4)),
    };
    // When the shared expression fold applies custom operator and reference spelling.
    let text = format_expression(&expression, &mut PrefixSyntax);
    // Then syntax reflects both folded operands exactly.
    assert_eq!(text, "(Add Parameter(\"n\") 4)");
}
