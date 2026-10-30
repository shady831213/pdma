//! The common walker drives structural and expression formatting callbacks.
use hw_model_schema::{
    export::{
        Artifact, Bindings, ChildCall, DefinitionHeader, GeometryAssignment, GeometryMember,
        GeometrySignature, InMemoryArtifactSink, PreparedSchema, Renderer, generate,
    },
    graph::{MapLayoutPlan, SchemaSet},
    register_map, register_schema,
};
register_schema! {pub register Word{width:4,endian:Little,access:ReadOnly,reset:Zero(0),writable_mask:0,fields:{}}}
register_map! {pub map Block{alignment:16,placements:[pub word:Word]}}
register_map! {pub map Root(n:usize){alignment:16,placements:[pub blocks:Block[n]]}}

#[derive(Default)]
struct Recording {
    root: bool,
    events: Vec<String>,
}
impl Renderer for Recording {
    type Error = std::convert::Infallible;
    fn begin_definition(&mut self, h: DefinitionHeader<'_>) -> Result<(), Self::Error> {
        self.root = h.name == "Root";
        Ok(())
    }
    fn geometry_member(&mut self, m: GeometryMember<'_>) -> Result<(), Self::Error> {
        if self.root {
            self.events.push(format!("member:{m:?}"));
        }
        Ok(())
    }
    fn geometry_parameter(&mut self, name: &str) -> Result<String, Self::Error> {
        if self.root {
            self.events.push(format!("parameter:{name}"));
        }
        Ok(format!("arg {name}"))
    }
    fn begin_geometry_body(
        &mut self,
        _: &MapLayoutPlan,
        s: &GeometrySignature,
    ) -> Result<(), Self::Error> {
        if self.root {
            assert_eq!(s.parameters, ["arg n"]);
            assert_eq!(s.arguments, ["p_n"]);
            self.events.push("body".into());
        }
        Ok(())
    }
    fn geometry_child_call(
        &mut self,
        c: ChildCall<'_>,
        args: &[String],
    ) -> Result<(), Self::Error> {
        if self.root {
            assert_eq!(c.name, "blocks");
            assert!(args.is_empty());
            self.events.push("child".into());
        }
        Ok(())
    }
    fn geometry_assignment(
        &mut self,
        a: GeometryAssignment<'_>,
        expr: &str,
    ) -> Result<(), Self::Error> {
        if self.root {
            self.events.push(format!("assign:{a:?}={expr}"));
        }
        Ok(())
    }
    fn finish(self) -> Result<Vec<Artifact>, Self::Error> {
        Ok(vec![Artifact {
            logical_name: "trace".into(),
            media_type: "text/plain".into(),
            contents: self.events.join("\n").into_bytes(),
        }])
    }
}
#[test]
fn generator_visits_headers_parameters_calls_and_assignments_in_order()
-> Result<(), Box<dyn std::error::Error>> {
    // Given: a symbolic root containing one bank of child layouts.
    let mut set = SchemaSet::new();
    set.add::<Root>();
    let prepared = PreparedSchema::from_graph(&set.resolve()?, &Bindings::new())?;
    let mut sink = InMemoryArtifactSink::new();
    // When: a custom renderer supplies only leaf formatting callbacks.
    generate(&prepared, Recording::default(), &mut sink)?;
    // Then: common traversal provides four fields, one parameter/call, and six assignments.
    let text = std::str::from_utf8(&sink.artifacts().first().ok_or("trace")?.contents)?;
    let lines = text.lines().collect::<Vec<_>>();
    assert!(lines.iter().take(4).all(|line| line.starts_with("member:")));
    assert_eq!(
        lines.get(4..7),
        Some(["parameter:n", "body", "child"].as_slice())
    );
    assert_eq!(
        lines
            .iter()
            .filter(|line| line.starts_with("assign:"))
            .count(),
        6
    );
    assert!(text.contains("=p_n"));
    assert!(text.contains("child_blocks.extent"));
    Ok(())
}

#[derive(Default)]
struct PrefixExpressions(Vec<String>);
impl Renderer for PrefixExpressions {
    type Error = std::convert::Infallible;
    fn expression_binary(
        &mut self,
        op: hw_model_schema::graph::ExpressionOp,
        left: &str,
        right: &str,
    ) -> String {
        format!("({op:?} {left} {right})")
    }
    fn expression_reference(
        &mut self,
        reference: hw_model_schema::export::ExpressionReference<'_>,
    ) -> String {
        format!("{reference:?}")
    }
    fn geometry_assignment(
        &mut self,
        _: GeometryAssignment<'_>,
        expression: &str,
    ) -> Result<(), Self::Error> {
        self.0.push(expression.into());
        Ok(())
    }
    fn finish(self) -> Result<Vec<Artifact>, Self::Error> {
        Ok(vec![Artifact {
            logical_name: "expressions".into(),
            media_type: "text/plain".into(),
            contents: self.0.join("\n").into_bytes(),
        }])
    }
}

#[test]
fn custom_syntax_uses_the_shared_recursive_fold() -> Result<(), Box<dyn std::error::Error>> {
    let mut set = SchemaSet::new();
    set.add::<Root>();
    let prepared = PreparedSchema::from_graph(&set.resolve()?, &Bindings::new())?;
    let mut sink = InMemoryArtifactSink::new();
    generate(&prepared, PrefixExpressions::default(), &mut sink)?;
    let text = std::str::from_utf8(&sink.artifacts().first().ok_or("expressions")?.contents)?;
    assert!(text.contains("(Add Placement { name: \"blocks\", property: Offset } (Multiply"));
    assert!(text.contains("Parameter(\"n\")"));
    assert!(!text.contains("result.blocks_"));
    Ok(())
}
