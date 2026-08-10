use crate::{register::RegisterSpec, register_map::RegisterMapSpec};

use super::{FragmentModelError, RegisterCHeaderModel, RegisterMapCHeaderModel};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Id {
    First,
    Second,
    Unknown,
}

const REGISTERS: [RegisterSpec<Id>; 2] = [
    RegisterSpec::new(Id::First, "first", 0, 4),
    RegisterSpec::new(Id::Second, "second", 4, 4),
];
const MAP: RegisterMapSpec<Id> = RegisterMapSpec::new(8, &REGISTERS, &[]);

#[test]
fn missing_typed_register_is_rejected() {
    let mut rows = RegisterMapCHeaderModel::from_spec(MAP).fixed;
    rows.pop();
    assert_eq!(
        RegisterMapCHeaderModel::try_from_parts(MAP, rows, Vec::new()),
        Err(FragmentModelError::MissingIdentity { expected_index: 1 })
    );
}

#[test]
fn duplicate_typed_register_identity_is_rejected() {
    let mut rows = RegisterMapCHeaderModel::from_spec(MAP).fixed;
    let first = rows.remove(0);
    rows = vec![first, first];
    assert_eq!(
        RegisterMapCHeaderModel::try_from_parts(MAP, rows, Vec::new()),
        Err(FragmentModelError::DuplicateIdentity {
            first_index: 0,
            duplicate_index: 1
        })
    );
}

#[test]
fn unknown_typed_register_identity_is_rejected() {
    let mut rows = RegisterMapCHeaderModel::from_spec(MAP).fixed;
    rows.pop();
    rows.push(RegisterCHeaderModel::from_spec(RegisterSpec::new(
        Id::Unknown,
        "unknown",
        4,
        4,
    )));
    assert_eq!(
        RegisterMapCHeaderModel::try_from_parts(MAP, rows, Vec::new()),
        Err(FragmentModelError::UnknownIdentity { actual_index: 1 })
    );
}

#[test]
fn reordered_typed_registers_are_rejected() {
    let mut rows = RegisterMapCHeaderModel::from_spec(MAP).fixed;
    rows.swap(0, 1);
    assert_eq!(
        RegisterMapCHeaderModel::try_from_parts(MAP, rows, Vec::new()),
        Err(FragmentModelError::ReorderedIdentity {
            actual_index: 0,
            expected_index: 1
        })
    );
}
