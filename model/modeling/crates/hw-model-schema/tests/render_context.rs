//! Bound visits preserve lexical ancestry when a target reorders or omits children.
use hw_model_schema::{
    analysis::{AnalyzedSchema, Bindings, CompleteMapLayout},
    ast::{SchemaSet, register::Parameter},
    export::{
        Artifact, InMemoryArtifactSink, generate,
        register::{self, EmitContext, LayoutProperty},
        value::{self, Backend},
    },
    register_map, register_schema,
};
register_schema! { pub register Word { width:4, endian:Little, access:ReadOnly, reset:Zero(0), writable_mask:0, fields:{} } }
register_map! { pub map Pair(n: usize) { alignment:4, placements:[pub zeta:Word[n], pub alpha:Word] } }
struct Probe {
    events: Vec<String>,
    fail: bool,
    binding: Option<usize>,
}
impl register::Backend for Probe {
    fn map_layout<'a>(
        &mut self,
        _: &'a CompleteMapLayout,
        ctx: EmitContext<'a>,
    ) -> Result<(), Self::Error> {
        assert_eq!(ctx.declared_parameters(), &["n"]);
        assert_eq!(
            ctx.parameters(),
            if self.binding.is_some() {
                &[][..]
            } else {
                &["n"][..]
            }
        );
        assert_eq!(
            ctx.bindings().and_then(|b| b.first().copied()),
            self.binding
        );
        let mut children: Vec<_> = ctx.children().collect();
        children.reverse();
        self.emit_all(children)
    }
    fn parameter(&mut self, p: Parameter, ctx: EmitContext<'_>) -> Result<(), Self::Error> {
        assert_eq!(ctx.declared_parameters().get(p.index), Some(&p.name));
        assert_eq!(
            p.binding,
            ctx.bindings().and_then(|b| b.get(p.index).copied())
        );
        self.events.push("parameter".into());
        Ok(())
    }
    fn layout_property<'a>(
        &mut self,
        property: LayoutProperty<'a>,
        ctx: EmitContext<'a>,
    ) -> Result<(), Self::Error> {
        assert_eq!(ctx.declared_parameters(), &["n"]);
        assert_eq!(
            ctx.parameters(),
            if self.binding.is_some() {
                &[][..]
            } else {
                &["n"][..]
            }
        );
        assert_eq!(
            ctx.bindings().and_then(|b| b.first().copied()),
            self.binding
        );
        match ctx.ancestors().last().ok_or("missing parent")? {
            register::Node::MapLayout(_) => assert_eq!(property.owner, None),
            register::Node::Placement(p) => {
                assert_eq!(property.owner, Some(p.reference.name()));
                assert_eq!(
                    ctx.resolve(&p.reference).ok_or("missing reference")?.name,
                    "Word"
                );
            }
            _ => return Err("wrong parent"),
        }
        self.events.push(format!(
            "{}:{}",
            property.owner.unwrap_or("root"),
            property.name
        ));
        if self.fail {
            return Err("layout rejected");
        }
        self.emit_all(ctx.children())
    }
}
impl value::Backend for Probe {
    type Domain = register::Domain;
    type Error = &'static str;
    type Context<'a> = EmitContext<'a>;
    fn finish(self) -> Result<Vec<Artifact>, Self::Error> {
        Ok(vec![Artifact {
            logical_name: "probe".into(),
            media_type: "text/plain".into(),
            contents: self.events.join("\n").into_bytes(),
        }])
    }
}
fn schema() -> Result<AnalyzedSchema, hw_model_schema::analysis::AnalysisError> {
    let mut set = SchemaSet::new();
    set.add::<Pair>();
    AnalyzedSchema::analyze(set, &Bindings::new())
}
#[test]
fn reverse_children_retain_parents_and_lexical_parameters() {
    let mut sink = InMemoryArtifactSink::new();
    generate(
        &schema().expect("valid schema"),
        Probe {
            events: vec![],
            fail: false,
            binding: None,
        },
        &mut sink,
    )
    .unwrap();
    let output =
        String::from_utf8(sink.artifacts().first().expect("artifact").contents.clone()).unwrap();
    assert_eq!(output.lines().next(), Some("root:extent"));
    assert_eq!(output.lines().last(), Some("parameter"));
    assert!(output.find("alpha:offset").unwrap() < output.find("zeta:offset").unwrap());
}
#[test]
fn typed_failure_publishes_no_artifacts() {
    let mut sink = InMemoryArtifactSink::new();
    assert!(matches!(
        generate(
            &schema().expect("valid schema"),
            Probe {
                events: vec![],
                binding: None,
                fail: true
            },
            &mut sink
        ),
        Err(hw_model_schema::export::ExportError::Render(
            "layout rejected"
        ))
    ));
    assert!(sink.artifacts().is_empty());
}

#[test]
fn bound_parameters_remain_available_before_parameter_visitation() {
    let mut set = SchemaSet::new();
    set.add::<Pair>();
    let mut bindings = Bindings::new();
    bindings.bind::<Pair>(&[3]);
    let schema = AnalyzedSchema::analyze(set, &bindings).unwrap();
    let mut sink = InMemoryArtifactSink::new();
    generate(
        &schema,
        Probe {
            events: vec![],
            fail: false,
            binding: Some(3),
        },
        &mut sink,
    )
    .unwrap();
    assert!(!sink.artifacts().is_empty());
}

#[test]
fn roots_are_bound_visits_with_default_recursive_dispatch() {
    use hw_model_schema::export::Domain;
    let schema = schema().unwrap();
    let roots = <register::Domain as Domain<Probe>>::roots(&schema);
    let mut probe = Probe {
        events: vec![],
        fail: false,
        binding: None,
    };
    for visit in roots {
        let ctx = visit.context();
        assert!(ctx.ancestors().is_empty());
        assert!(core::ptr::eq(ctx.analysis(), core::ptr::from_ref(&schema)));
        assert!(
            matches!(visit.node(), register::Node::Root(unit) if core::ptr::eq(unit, ctx.unit()))
        );
        <register::Domain as Domain<Probe>>::root(visit, &mut probe).unwrap();
    }
    assert_eq!(
        probe.events.first().map(String::as_str),
        Some("root:extent")
    );
    assert_eq!(probe.events.last().map(String::as_str), Some("parameter"));
}

#[test]
fn rendering_failure_preserves_existing_sink_artifacts() {
    use hw_model_schema::export::ArtifactSink;
    let mut sink = InMemoryArtifactSink::new();
    let existing = Artifact {
        logical_name: "existing".into(),
        media_type: "text/plain".into(),
        contents: b"retained".to_vec(),
    };
    sink.emit(existing.clone()).unwrap();
    let result = generate(
        &schema().unwrap(),
        Probe {
            events: vec![],
            fail: true,
            binding: None,
        },
        &mut sink,
    );
    assert!(matches!(
        result,
        Err(hw_model_schema::export::ExportError::Render(
            "layout rejected"
        ))
    ));
    assert_eq!(sink.artifacts(), &[existing]);
}
