//! Bound visits preserve lexical ancestry when a target reorders or omits children.
/// Caller-owned capture for generation tests.
pub mod output_support;
use hw_model_schema::export::register::{Context as RegisterContext, LayoutProperty};
use hw_model_schema::{
    analysis::{AnalyzedSchema, Bindings},
    ast::{SchemaSet, register::Parameter},
    export::{
        generate, register,
        value::{self, Emit as _},
    },
    register_map, register_schema,
};
use output_support::MemoryOutput;
register_schema! { pub register Word { width:4, endian:Little, access:ReadOnly, reset:Zero(0), writable_mask:0, fields:{} } }
register_map! { pub map Pair(n: usize) { alignment:4, placements:[pub zeta:Word[n], pub alpha:Word] } }
/// Custom backend used to observe traversal context.
pub mod register_context_probe;
use register_context_probe::Probe;
fn schema() -> Result<AnalyzedSchema, hw_model_schema::analysis::AnalysisError> {
    let mut set = SchemaSet::new();
    set.add::<Pair>();
    AnalyzedSchema::analyze(set, &Bindings::new())
}
#[test]
fn reverse_children_retain_parents_and_lexical_parameters() {
    let mut sink = MemoryOutput::new();
    sink.generate(
        &schema().expect("valid schema"),
        Probe {
            events: vec![],
            fail: false,
            binding: None,
        },
    )
    .unwrap();
    let output = String::from_utf8(
        sink.outputs()
            .iter()
            .flat_map(|output| output.bytes.iter().copied())
            .collect::<Vec<_>>(),
    )
    .unwrap();
    assert_eq!(output.lines().next(), Some("root:extent"));
    assert_eq!(output.lines().last(), Some("parameter"));
    assert!(output.find("alpha:offset").unwrap() < output.find("zeta:offset").unwrap());
}
#[test]
fn typed_failure_preserves_completed_dependency() {
    let mut sink = MemoryOutput::new();
    assert!(matches!(
        sink.generate(
            &schema().expect("valid schema"),
            Probe {
                events: vec![],
                binding: None,
                fail: true
            },
        ),
        Err(hw_model_schema::export::ExportError::Render(
            "layout rejected"
        ))
    ));
    assert_eq!(sink.outputs().len(), 1);
    let completed = sink.outputs().first().expect("completed dependency");
    assert_eq!(completed.entry_name, "Word");
    assert!(completed.bytes.is_empty());
}

#[test]
fn bound_parameters_remain_available_before_parameter_visitation() {
    let mut set = SchemaSet::new();
    set.add::<Pair>();
    let mut bindings = Bindings::new();
    bindings.bind::<Pair>(&[3]);
    let schema = AnalyzedSchema::analyze(set, &bindings).unwrap();
    let mut sink = MemoryOutput::new();
    sink.generate(
        &schema,
        Probe {
            events: vec![],
            fail: false,
            binding: Some(3),
        },
    )
    .unwrap();
    assert!(!sink.outputs().is_empty());
    let output = String::from_utf8(
        sink.outputs()
            .iter()
            .flat_map(|output| output.bytes.iter().copied())
            .collect::<Vec<_>>(),
    )
    .unwrap();
    assert_eq!(
        output
            .lines()
            .filter(|line| line.starts_with("bank:"))
            .collect::<Vec<_>>(),
        [
            "bank:zeta:offset=0",
            "bank:zeta:count=3",
            "bank:zeta:stride=4"
        ]
    );
}

#[test]
fn roots_are_bound_visits_with_default_recursive_dispatch() {
    use hw_model_schema::export::Domain;
    let schema = schema().unwrap();
    let roots = <register::Domain as Domain<Probe>>::entries(&schema);
    let mut probe = Probe {
        events: vec![],
        fail: false,
        binding: None,
    };
    for visit in roots {
        assert!(visit.parent_id().is_none());
        assert_eq!(visit.kind(), hw_model_schema::export::Kind::Entry);
        assert!(schema.units().any(|unit| &unit.id == visit.id()));
        <register::Domain as Domain<Probe>>::entry(visit, &mut probe, &mut std::io::sink())
            .unwrap();
    }
    assert_eq!(
        probe.events.first().map(String::as_str),
        Some("root:extent")
    );
    assert_eq!(probe.events.last().map(String::as_str), Some("parameter"));
}

#[test]
fn rendering_failure_preserves_existing_bytes_and_partial_direct_writes() {
    let mut sink = b"retained".to_vec();
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
    assert_eq!(sink, b"retainedroot:extent");
}
