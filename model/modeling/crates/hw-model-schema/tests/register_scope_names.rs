//! Sibling-local register placement-name validation coverage.

use hw_model_schema::Endian;
use hw_model_schema::register::{RegisterDefinition, RegisterSpec};
use hw_model_schema::register_map::{
    RegisterMapError, RegisterMapSpec, RepeatedRegisterBank, ResolvedRegister,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RegisterId {
    Fixed,
    First,
    Second,
}

const VALUE: RegisterDefinition<u16> = RegisterDefinition::new("Value", 4, Endian::Little);

#[test]
fn identical_names_in_distinct_sibling_scopes_resolve_to_distinct_paths() {
    // Given
    const FIXED: [RegisterSpec<RegisterId>; 1] =
        [RegisterSpec::place(RegisterId::Fixed, "control", 0, VALUE)];
    const MEMBERS: [RegisterSpec<RegisterId>; 1] =
        [RegisterSpec::place(RegisterId::First, "control", 0, VALUE)];
    const BANKS: [RepeatedRegisterBank<RegisterId>; 1] =
        [RepeatedRegisterBank::new("channel", 16, 2, 8, &MEMBERS)];

    // When
    let resolved = RegisterMapSpec::named("Device", 32, &FIXED, &BANKS).resolve();

    // Then
    let resolved = resolved.expect("names in distinct scopes must resolve");
    let paths = resolved
        .registers()
        .iter()
        .map(ResolvedRegister::path)
        .collect::<Vec<_>>();
    assert_eq!(
        paths,
        [
            "Device.control",
            "Device.channel[0].control",
            "Device.channel[1].control",
        ]
    );
    println!("resolved_paths={paths:?}");
}

#[test]
fn duplicate_names_within_fixed_scope_report_the_later_path() {
    // Given
    const FIXED: [RegisterSpec<RegisterId>; 2] = [
        RegisterSpec::place(RegisterId::Fixed, "control", 0, VALUE),
        RegisterSpec::place(RegisterId::First, "control", 4, VALUE),
    ];

    // When
    let result = RegisterMapSpec::named("Device", 16, &FIXED, &[]).resolve();

    // Then
    println!("fixed_scope_error={result:?}");
    assert_eq!(
        result,
        Err(RegisterMapError::DuplicatePlacementName {
            root: "Device",
            path: "Device.control".into(),
            declaration_index: 1,
            name: "control",
        })
    );
}

#[test]
fn duplicate_names_within_one_bank_scope_report_the_later_path() {
    // Given
    const MEMBERS: [RegisterSpec<RegisterId>; 2] = [
        RegisterSpec::place(RegisterId::First, "control", 0, VALUE),
        RegisterSpec::place(RegisterId::Second, "control", 4, VALUE),
    ];
    const BANKS: [RepeatedRegisterBank<RegisterId>; 1] =
        [RepeatedRegisterBank::new("channel", 16, 1, 8, &MEMBERS)];

    // When
    let result = RegisterMapSpec::named("Device", 32, &[], &BANKS).resolve();

    // Then
    println!("bank_scope_error={result:?}");
    assert_eq!(
        result,
        Err(RegisterMapError::DuplicatePlacementName {
            root: "Device",
            path: "Device.channel[0].control".into(),
            declaration_index: 1,
            name: "control",
        })
    );
}

#[test]
fn projected_bank_and_register_paths_share_source_segments() {
    use hw_model_schema::{
        analysis::{AnalyzedSchema, ViewPathSegment},
        ast::RegisterLocation,
        traversal::register::{self, Context, Node},
    };
    fn segments(parts: &[ViewPathSegment]) -> Vec<(&str, Option<usize>)> {
        parts
            .iter()
            .map(|part| (part.name.as_str(), part.index))
            .collect()
    }
    fn walk(ctx: &Context<'_>, seen: &mut Vec<RegisterLocation>) {
        match ctx.node() {
            Node::Bank(bank) => {
                assert_eq!(bank.path(), "Device.channel");
                assert_eq!(segments(bank.segments()), [("channel", None)]);
            }
            Node::RegisterPlacement { view, location } => {
                let index = match location {
                    RegisterLocation::Template { .. } => None,
                    RegisterLocation::Instance { instance, .. } => Some(instance),
                    _ => panic!("expected a bank member"),
                };
                assert_eq!(
                    segments(&view.segments),
                    [("channel", index), ("control", None)]
                );
                let path = index.map_or_else(
                    || "Device.channel.control".to_owned(),
                    |index| format!("Device.channel[{index}].control"),
                );
                assert_eq!(view.path, path);
                seen.push(location);
            }
            _ => {}
        }
        for child in ctx.children() {
            walk(child.context(), seen);
        }
    }
    const MEMBERS: [RegisterSpec<RegisterId>; 1] =
        [RegisterSpec::place(RegisterId::First, "control", 0, VALUE)];
    const BANKS: [RepeatedRegisterBank<RegisterId>; 1] =
        [RepeatedRegisterBank::new("channel", 16, 2, 8, &MEMBERS)];
    let resolved = RegisterMapSpec::named("Device", 32, &[], &BANKS)
        .resolve()
        .unwrap();
    let schema = AnalyzedSchema::register_map("unrelated target alias", &resolved).unwrap();
    let mut seen = Vec::new();
    for root in register::roots(&schema) {
        walk(root.context(), &mut seen);
    }
    assert_eq!(seen.len(), 3);
}
