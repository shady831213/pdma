//! Analyzed map programs preserve dependency order before target rendering.
use hw_model_schema::{
    ast::{
        DefinitionId, SchemaSet,
        analysis::{
            Analysis, Bindings, Layout,
            program::{Expression, LayoutProperty, Member, Statement, Symbol},
        },
    },
    register_map, register_schema,
};

register_schema! { pub register Word { width:4, endian:Little, access:ReadOnly,
reset:Zero(0), writable_mask:0, fields:{} } }
register_map! { pub map Child(n: usize) { alignment:4, placements:[pub words:Word[n]] } }
register_map! { pub map Parent(n: usize) { alignment:16, placements:[
    pub head:Word, pub groups:Child(n)[2], pub tail:Word,
] } }

fn analyze(bindings: &Bindings) -> Result<Analysis, Box<dyn std::error::Error>> {
    let mut set = SchemaSet::new();
    set.add::<Parent>();
    Ok(Analysis::from_graph(&set.resolve()?, bindings)?)
}

#[test]
fn symbolic_program_defines_child_results_before_placement_and_final_extent() {
    // Given: a parent with a parameterized child between two fixed registers.
    let analysis = analyze(&Bindings::new()).expect("valid analysis");
    let root = analysis.definitions().last().expect("parent");
    // When: a backend reads the complete analyzed layout program.
    let Layout::Map(map) = &root.body else {
        panic!("map expected")
    };
    let program = map.program.as_ref().expect("program");
    // Then: execution order is already determined without inspecting placement formulas.
    assert_eq!(program.parameters, ["n"]);
    assert_eq!(program.locals, [Symbol::Cursor]);
    let mut available_child = false;
    let mut saw_group_offset = false;
    for statement in &program.body {
        match statement {
            Statement::Call(call) => {
                assert_eq!(call.definition, DefinitionId::of::<Child>());
                assert_eq!(
                    call.arguments,
                    [Expression::Reference(Symbol::Parameter("n"))]
                );
                available_child = true;
            }
            Statement::Assign {
                target:
                    Symbol::Placement {
                        name: "groups",
                        property: LayoutProperty::Offset,
                    },
                ..
            } => {
                assert!(available_child);
                saw_group_offset = true;
            }
            Statement::Define {
                target: Symbol::End("groups"),
                ..
            } => assert!(saw_group_offset),
            Statement::Assign { .. } | Statement::Define { .. } => {}
        }
    }
    assert!(saw_group_offset);
    assert!(matches!(
        program.body.last(),
        Some(Statement::Assign {
            target: Symbol::Extent,
            value: Expression::Reference(Symbol::Cursor),
        })
    ));
    assert_eq!(program.members.first(), Some(&Member::Extent));
    assert_eq!(program.members.len(), 1 + 3 * 3);
}

#[test]
fn bound_program_contains_only_concrete_result_assignments() {
    // Given: the same map bound at three distinct sizes.
    for n in [1, 3, 5] {
        let mut bindings = Bindings::new();
        bindings.bind::<Parent>(&[n]);
        // When: common analysis prepares the bound root.
        let analysis = analyze(&bindings).expect("valid analysis");
        let root = analysis.definitions().last().expect("parent");
        let Layout::Map(map) = &root.body else {
            panic!("map expected")
        };
        let program = map.program.as_ref().expect("program");
        // Then: no child call or cursor work remains, and values agree with the real constructor.
        assert!(program.parameters.is_empty());
        assert!(program.locals.is_empty());
        assert!(program.body.iter().all(|statement| matches!(
            statement,
            Statement::Assign {
                value: Expression::Literal(_),
                ..
            }
        )));
        let expected = Parent::new(n).expect("valid instance");
        assert!(program.body.contains(&Statement::Assign {
            target: Symbol::Extent,
            value: Expression::Literal(expected.extent()),
        }));
        assert!(program.body.contains(&Statement::Assign {
            target: Symbol::Placement {
                name: "groups",
                property: LayoutProperty::Stride
            },
            value: Expression::Literal(expected.groups().stride()),
        }));
    }
}
