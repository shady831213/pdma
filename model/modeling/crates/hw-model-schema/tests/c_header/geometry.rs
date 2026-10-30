use hw_model_schema::ast::analysis::{Analysis, Bindings};
use hw_model_schema::{
    ast::{DefinitionId, SchemaSet},
    export::backends::c_header::{CHeaderBackend, CHeaderExportError},
    export::{ExportError, InMemoryArtifactSink, generate},
    register_map, register_schema,
};

register_schema! { pub register Word { width: 4, endian: Little, access: ReadOnly,
reset: Zero(0), writable_mask: 0, fields: {} } }
register_map! { pub map Child(n: usize) { alignment: 4, placements: [words: Word[n]] } }
register_map! { pub map Parent(n: usize) { alignment: 16, placements: [
    header: Word, groups: Child(n)[2], tail: Word,
] } }

#[test]
fn common_preparation_renders_symbolic_and_bound_nested_geometry_as_strict_c11()
-> super::support::TestResult {
    // Given: the same parameterized map graph is prepared symbolically and at n=3.
    let mut selected = SchemaSet::new();
    selected.add::<Parent>();
    let graph = selected.resolve()?;
    let mut bound = Bindings::new();
    bound.bind::<Parent>(&[3]);

    for (label, bindings) in [("symbolic", Bindings::new()), ("bound", bound)] {
        let prepared = Analysis::from_graph(&graph, &bindings)?;
        let renderer = CHeaderBackend::new()
            .with_name(DefinitionId::of::<Parent>(), "root")
            .with_name(DefinitionId::of::<Child>(), "leaf");
        let mut sink = InMemoryArtifactSink::new();

        // When: generation visits prepared definitions and C only renders their formulas.
        generate(&prepared, renderer, &mut sink)?;
        let fragments: Vec<_> = sink
            .artifacts()
            .iter()
            .map(|artifact| artifact.contents.as_slice())
            .collect();

        // Then: native C observes independently calculated geometry at three parameter values.
        let consumer: &[u8] = if label == "symbolic" {
            b"int main(void) {\n    C_ROOT_Geometry one = C_ROOT_GEOMETRY(1);\n    C_ROOT_Geometry three = C_ROOT_GEOMETRY(3);\n    C_ROOT_Geometry five = C_ROOT_GEOMETRY(5);\n    return one.extent != 16 || one.C_GROUPS_stride != 4 || three.extent != 32 || three.C_GROUPS_offset != 4 || three.C_GROUPS_count != 2 || three.C_GROUPS_stride != 12 || three.C_TAIL_offset != 28 || five.extent != 48 || five.C_TAIL_offset != 44;\n}\n"
        } else {
            b"int main(void) {\n    C_ROOT_Geometry root = C_ROOT_GEOMETRY();\n    C_LEAF_Geometry one = C_LEAF_GEOMETRY(1);\n    C_LEAF_Geometry three = C_LEAF_GEOMETRY(3);\n    C_LEAF_Geometry five = C_LEAF_GEOMETRY(5);\n    return root.extent != 32 || C_ROOT_SIZE != 32 || root.C_GROUPS_offset != 4 || root.C_GROUPS_count != 2 || root.C_GROUPS_stride != 12 || root.C_TAIL_offset != 28 || one.extent != 4 || three.extent != 12 || five.extent != 20;\n}\n"
        };
        super::support::compile_fragments(label, &fragments, consumer)?;
    }
    Ok(())
}

#[test]
fn symbolic_geometry_rejects_aliasing_type_and_function_names() -> super::support::TestResult {
    // Given: a symbolic map with a target prefix longer than C11's significant length.
    let mut selected = SchemaSet::new();
    selected.add::<Parent>();
    let prepared = Analysis::from_graph(&selected.resolve()?, &Bindings::new())?;
    let prefix = "a".repeat(64);
    let renderer = CHeaderBackend::new().with_name(DefinitionId::of::<Parent>(), &prefix);
    let mut sink = InMemoryArtifactSink::new();

    // When: its geometry type and function would alias in the ordinary C namespace.
    let failure = generate(&prepared, renderer, &mut sink);

    // Then: the exact declarations are diagnosed before any artifact reaches the sink.
    let stem = format!("C_{}", prefix.to_ascii_uppercase());
    assert_eq!(
        failure,
        Err(ExportError::Render(CHeaderExportError::NameCollision {
            first_path: format!("{stem}_Geometry"),
            second_path: format!("{stem}_GEOMETRY"),
            first_index: 0,
            second_index: 1,
        }))
    );
    assert!(sink.artifacts().is_empty());
    Ok(())
}
