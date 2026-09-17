//! Complete-envelope validation for manually assembled register maps.
use hardware_interface_schema::{
    RegisterMapError, RegisterMapSpec, RegisterMapValidationError, RegisterSpec,
    RepeatedRegisterBank,
};

const MEMBERS: [RegisterSpec<u16>; 2] = [
    RegisterSpec::new(1, "control", 0, 4),
    RegisterSpec::new(2, "status", 8, 4),
];
const BANK: [RepeatedRegisterBank<u16>; 1] =
    [RepeatedRegisterBank::new("channels", 16, 2, 16, &MEMBERS)];
const INTRUDER: [RegisterSpec<u16>; 1] = [RegisterSpec::new(3, "intruder", 20, 4)];
const GAP: [RegisterSpec<u16>; 1] = [RegisterSpec::new(3, "alignment_gap", 28, 4)];
const INNER_BANK: [RegisterSpec<u16>; 1] = [RegisterSpec::new(3, "intruder", 0, 4)];
const INTERLEAVED: [RepeatedRegisterBank<u16>; 2] = [
    RepeatedRegisterBank::new("channels", 16, 2, 16, &MEMBERS),
    RepeatedRegisterBank::new("intruders", 20, 2, 16, &INNER_BANK),
];

#[test]
fn both_manual_validation_paths_reject_a_scalar_inside_bank_holes() {
    for scalar in [&INTRUDER, &GAP] {
        let map = RegisterMapSpec::named("device", 64, scalar, &BANK);
        assert!(matches!(
            map.validate(),
            Err(RegisterMapValidationError::PlacementEnvelopeOverlap { .. })
        ));
        assert!(matches!(
            map.resolve(),
            Err(RegisterMapError::PlacementEnvelopeOverlap { .. })
        ));
    }
}

#[test]
fn both_manual_validation_paths_reject_interleaved_bank_envelopes() {
    let map = RegisterMapSpec::named("device", 64, &[], &INTERLEAVED);
    assert!(matches!(
        map.validate(),
        Err(RegisterMapValidationError::PlacementEnvelopeOverlap { .. })
    ));
    assert!(matches!(
        map.resolve(),
        Err(RegisterMapError::PlacementEnvelopeOverlap { .. })
    ));
}
