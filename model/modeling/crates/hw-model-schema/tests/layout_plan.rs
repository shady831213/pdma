//! Shared symbolic layout agrees with generated constructors.
use hw_model_schema::ast::{
    ChildLayout as PlannedChild, DefinitionBody, DescribeSchema, LayoutError, LayoutExpr,
};
use hw_model_schema::{bits_schema, register_map, register_schema};
bits_schema! { pub bits Flags(u8) { enabled: 0..1, } }
register_schema! { pub register Word { width: 4, endian: Little, access: ReadWrite, reset: Zero(0), writable_mask: 0xffff_ffff, fields: {} } }
register_map! { pub map Child(n: usize) { alignment: 16, placements: [pub words: Word[n]] } }
register_map! { pub map Root(n: usize) { alignment: 16, placements: [pub head: Word, pub children: Child(n)[2]] } }
#[test]
fn nested_banks_preserve_holes_and_last_child_extent() -> Result<(), Box<dyn std::error::Error>> {
    // Given: a parameterized child bank preceded by an alignment hole.
    let definition = Root::schema_definition();
    let DefinitionBody::RegisterMap(map) = definition.body() else {
        return Err("map expected".into());
    };
    let plan = map.layout_plan()?;
    let child = plan.placements().get(1).ok_or("missing planned child")?;
    let PlannedChild::Call {
        arguments,
        plan: nested,
    } = &child.child
    else {
        return Err("map call expected".into());
    };
    assert_eq!(arguments, &[LayoutExpr::Parameter("n")]);
    assert_eq!(nested.parameters(), &["n"]);
    assert_eq!(child.child_name, "Child");
    for n in [1, 3, 5] {
        // When: the common plan and generated constructor receive identical bindings.
        let geometry = plan.evaluate(&[n])?;
        let actual = Root::new(n)?;
        let bound = plan.bind(&[n])?;
        assert!(bound.parameters().is_empty());
        assert_eq!(bound.evaluate(&[])?, geometry);
        let bound_child = bound.placements().get(1).ok_or("missing bound child")?;
        assert_eq!(bound_child.child_definition, child.child_definition);
        assert!(
            matches!(bound_child.child, PlannedChild::Fixed { alignment: 16, extent } if extent == n * 4)
        );
        for placement in bound.placements() {
            for expr in [
                &placement.offset,
                &placement.count,
                &placement.stride,
                &placement.end,
                &placement.cursor,
                bound.extent(),
            ] {
                assert!(matches!(expr, LayoutExpr::Constant(_)));
            }
        }
        // Then: every bank property and the unpadded final extent agree.
        assert_eq!(geometry.extent, actual.extent());
        let bank = geometry.placements.get(1).ok_or("missing child bank")?;
        assert_eq!(
            (bank.offset, bank.count, bank.stride),
            (
                actual.children().base(),
                actual.children().count(),
                actual.children().stride()
            )
        );
        assert_eq!(geometry.extent, bank.offset + bank.stride + n * 4);
    }
    assert_eq!(plan.evaluate(&[]), Err(LayoutError::Arguments));
    assert_eq!(plan.evaluate(&[0]), Err(LayoutError::Geometry));
    assert_eq!(plan.evaluate(&[usize::MAX]), Err(LayoutError::Geometry));
    Ok(())
}

#[test]
fn malformed_layouts_preserve_checked_errors() -> Result<(), Box<dyn std::error::Error>> {
    use hw_model_schema::ast::{LayoutExpr, MapDefinition, MapPlacement};
    // Given: explicit offsets and counts can describe malformed envelopes.
    let place = |offset, count| {
        MapPlacement::new(
            "word",
            <Word as hw_model_schema::RegisterLayout>::schema_definition,
            vec![],
            LayoutExpr::Constant(count),
            offset,
            true,
        )
    };
    // When evaluated, then each malformed envelope returns the established error.
    for map in [
        MapDefinition::new(&[], 3, None, vec![place(None, 1)]),
        MapDefinition::new(&[], 4, Some(3), vec![place(None, 1)]),
        MapDefinition::new(&[], 4, None, vec![place(Some(1), 1)]),
        MapDefinition::new(&[], 4, None, vec![place(Some(usize::MAX - 3), 1)]),
        MapDefinition::new(&[], 4, None, vec![place(None, 0)]),
    ] {
        assert_eq!(
            map.layout_plan().and_then(|plan| plan.evaluate(&[])),
            Err(LayoutError::Geometry)
        );
    }
    let overlap = MapDefinition::new(&[], 4, None, vec![place(Some(8), 2), place(Some(12), 1)]);
    assert_eq!(
        overlap.layout_plan().and_then(|plan| plan.evaluate(&[])),
        Err(LayoutError::Overlap)
    );
    let holes = MapDefinition::new(
        &[],
        4,
        Some(32),
        vec![place(Some(16), 1), place(Some(0), 1)],
    );
    let geometry = holes.layout_plan()?.evaluate(&[])?;
    assert_eq!(geometry.extent, 32);
    assert_eq!(
        geometry
            .placements
            .get(1)
            .ok_or("missing second placement")?
            .offset,
        0
    );
    Ok(())
}

struct Cycle;
impl DescribeSchema for Cycle {
    fn schema_definition() -> hw_model_schema::ast::Definition {
        use hw_model_schema::ast::{Definition, LayoutExpr, MapDefinition, MapPlacement};
        Definition::register_map::<Self>(
            "Cycle",
            hw_model_schema::Documentation::EMPTY,
            MapDefinition::new(
                &[],
                4,
                None,
                vec![MapPlacement::new(
                    "again",
                    Self::schema_definition,
                    vec![],
                    LayoutExpr::Constant(1),
                    None,
                    false,
                )],
            ),
        )
    }
}
#[test]
fn recursive_layouts_are_rejected() {
    // Given: a map references itself by value.
    let definition = Cycle::schema_definition();
    let DefinitionBody::RegisterMap(map) = definition.body() else {
        panic!("map expected")
    };
    // When evaluated, then recursive geometry is rejected without unbounded recursion.
    assert!(matches!(
        map.layout_plan().and_then(|plan| plan.evaluate(&[])),
        Err(LayoutError::Cycle(_))
    ));
}

#[test]
fn invalid_bindings_and_child_kinds_keep_their_errors() {
    use hw_model_schema::ast::{LayoutExpr, MapDefinition, MapPlacement};
    // Given: invalid child bindings and unsupported count expressions.
    let cases = [
        (
            MapPlacement::new(
                "bits",
                hw_model_schema::ast::Definition::bits::<Flags>,
                vec![],
                LayoutExpr::Constant(1),
                None,
                false,
            ),
            LayoutError::Geometry,
        ),
        (
            MapPlacement::new(
                "child",
                Child::schema_definition,
                vec![],
                LayoutExpr::Constant(1),
                None,
                false,
            ),
            LayoutError::Arguments,
        ),
        (
            MapPlacement::new(
                "word",
                <Word as hw_model_schema::RegisterLayout>::schema_definition,
                vec![LayoutExpr::Constant(1)],
                LayoutExpr::Constant(1),
                None,
                false,
            ),
            LayoutError::Geometry,
        ),
        (
            MapPlacement::new(
                "word",
                <Word as hw_model_schema::RegisterLayout>::schema_definition,
                vec![],
                LayoutExpr::Parameter("absent"),
                None,
                false,
            ),
            LayoutError::Parameter("absent"),
        ),
        (
            MapPlacement::new(
                "word",
                <Word as hw_model_schema::RegisterLayout>::schema_definition,
                vec![],
                LayoutExpr::Unsupported("custom()"),
                None,
                false,
            ),
            LayoutError::Unsupported("custom()"),
        ),
    ];
    // When planned without instance values, then each preserves its specific diagnostic.
    for (placement, error) in cases {
        assert_eq!(
            MapDefinition::new(&["n"], 16, None, vec![placement])
                .layout_plan()
                .err(),
            Some(error)
        );
    }
}

#[test]
fn symbolic_planning_rejects_unbound_child_arguments() {
    use hw_model_schema::ast::{LayoutExpr, MapDefinition, MapPlacement};
    // Given: a symbolic map forwards an undeclared parameter to a valid child.
    let map = MapDefinition::new(
        &["n"],
        16,
        None,
        vec![MapPlacement::new(
            "child",
            Child::schema_definition,
            vec![LayoutExpr::Parameter("missing")],
            LayoutExpr::Constant(1),
            None,
            false,
        )],
    );
    // When planning without an instance, then the missing binding is already rejected.
    assert!(matches!(
        map.layout_plan(),
        Err(LayoutError::Parameter("missing"))
    ));
}
