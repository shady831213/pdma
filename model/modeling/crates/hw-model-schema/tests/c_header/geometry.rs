use hw_model_schema::analysis::{AnalyzedSchema, Bindings};
use hw_model_schema::{
    ast::{DefinitionId, SchemaSet},
    export::backends::c_header::{CHeaderExportError, CRegisterBackend},
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
        let prepared = AnalyzedSchema::from_graph(&graph, &bindings)?;
        let renderer = CRegisterBackend::new()
            .with_name(
                prepared
                    .definition(DefinitionId::of::<Parent>())
                    .ok_or("missing definition")?
                    .id
                    .clone(),
                "root",
            )
            .with_name(
                prepared
                    .definition(DefinitionId::of::<Child>())
                    .ok_or("missing definition")?
                    .id
                    .clone(),
                "leaf",
            );
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
    let prepared = AnalyzedSchema::from_graph(&selected.resolve()?, &Bindings::new())?;
    let prefix = "a".repeat(64);
    let renderer = CRegisterBackend::new().with_name(
        prepared
            .definition(DefinitionId::of::<Parent>())
            .ok_or("missing definition")?
            .id
            .clone(),
        &prefix,
    );
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

register_map! { pub map EmptyImplicit(_n: usize) { alignment: 4, placements: [] } }
register_map! { pub map EmptyExplicit(_n: usize) { alignment: 4, extent: 8, placements: [] } }

#[test]
fn empty_parameterized_maps_compile_and_preserve_implicit_and_explicit_extents()
-> super::support::TestResult {
    // Given: two empty maps retain an unused constructor parameter.
    let mut selected = SchemaSet::new();
    selected.add::<EmptyImplicit>();
    selected.add::<EmptyExplicit>();
    let prepared = AnalyzedSchema::from_graph(&selected.resolve()?, &Bindings::new())?;
    let renderer = CRegisterBackend::new()
        .with_name(
            prepared
                .definition(DefinitionId::of::<EmptyImplicit>())
                .ok_or("missing definition")?
                .id
                .clone(),
            "implicit",
        )
        .with_name(
            prepared
                .definition(DefinitionId::of::<EmptyExplicit>())
                .ok_or("missing definition")?
                .id
                .clone(),
            "explicit",
        );
    let mut sink = InMemoryArtifactSink::new();
    // When: their symbolic geometry is emitted for a strict native C11 consumer.
    generate(&prepared, renderer, &mut sink)?;
    let fragments: Vec<_> = sink
        .artifacts()
        .iter()
        .map(|artifact| artifact.contents.as_slice())
        .collect();
    // Then: implicit extent remains zero and explicit extent remains eight for any input.
    super::support::compile_fragments("empty-parameterized", &fragments,
        b"int main(void) { return C_IMPLICIT_GEOMETRY(0).extent != 0 || C_IMPLICIT_GEOMETRY(17).extent != 0 || C_EXPLICIT_GEOMETRY(0).extent != 8 || C_EXPLICIT_GEOMETRY(17).extent != 8; }\n")
}

register_map! { pub map Independent(width: usize, depth: usize) {
    alignment: 4, placements: [columns: Word[width], rows: Word[depth]],
} }

#[test]
fn geometry_parameters_remain_local_across_bound_and_unbound_definitions()
-> super::support::TestResult {
    // Given: a bound nested map precedes an independent map with two lexical parameters.
    let mut selected = SchemaSet::new();
    selected.add::<Parent>();
    selected.add::<Independent>();
    let mut bindings = Bindings::new();
    bindings.bind::<Parent>(&[3]);
    let prepared = AnalyzedSchema::from_graph(&selected.resolve()?, &bindings)?;
    let renderer = CRegisterBackend::new()
        .with_name(
            prepared
                .definition(DefinitionId::of::<Parent>())
                .ok_or("missing definition")?
                .id
                .clone(),
            "root",
        )
        .with_name(
            prepared
                .definition(DefinitionId::of::<Child>())
                .ok_or("missing definition")?
                .id
                .clone(),
            "leaf",
        )
        .with_name(
            prepared
                .definition(DefinitionId::of::<Independent>())
                .ok_or("missing definition")?
                .id
                .clone(),
            "independent",
        );
    let mut sink = InMemoryArtifactSink::new();

    // When: one backend emits all definitions into a native C consumer.
    generate(&prepared, renderer, &mut sink)?;
    let fragments: Vec<_> = sink
        .artifacts()
        .iter()
        .map(|artifact| artifact.contents.as_slice())
        .collect();

    // Then: each function accepts only its own ordered, unbound parameters.
    super::support::compile_fragments(
        "lexical-parameters",
        &fragments,
        br"
int main(void) {
    C_INDEPENDENT_Geometry first = C_INDEPENDENT_GEOMETRY(2, 5);
    C_INDEPENDENT_Geometry second = C_INDEPENDENT_GEOMETRY(5, 2);
    return C_ROOT_GEOMETRY().extent != 32 || C_LEAF_GEOMETRY(7).extent != 28
        || first.C_COLUMNS_count != 2 || first.C_ROWS_count != 5
        || first.C_ROWS_offset != 8 || first.extent != 28
        || second.C_COLUMNS_count != 5 || second.C_ROWS_count != 2
        || second.C_ROWS_offset != 20 || second.extent != 28;
}
",
    )
}
