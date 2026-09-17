//! Runtime-neutral permission projection preserves physical register geometry.
use hardware_interface_schema::{Access, RegisterMapSpec, RegisterSpec, RepeatedRegisterBank};

const MEMBERS: [RegisterSpec<u8>; 2] = [
    RegisterSpec::new(1, "private", 0, 4),
    RegisterSpec::new(2, "public", 8, 4)
        .with_access(Access::ReadWrite)
        .with_writable_mask(7),
];
const BANKS: [RepeatedRegisterBank<u8>; 1] =
    [RepeatedRegisterBank::new("ports", 32, 3, 16, &MEMBERS)];

#[test]
fn observer_projection_preserves_bank_geometry() {
    // Given: an arbitrary observer policy unrelated to any device or runtime.
    let map = RegisterMapSpec::named("sensor", 128, &[], &BANKS)
        .resolve()
        .unwrap();
    // When: private members are hidden and public members become read-only.
    let view = map
        .project_access(|register| match register.id() {
            2 => Some(Access::ReadOnly),
            _ => None,
        })
        .unwrap();
    // Then: holes, indices, extent and bank stride survive without compaction.
    assert_eq!(view.size(), 128);
    assert_eq!(
        view.registers()
            .iter()
            .map(|r| (r.offset(), r.instance()))
            .collect::<Vec<_>>(),
        [(40, Some(0)), (56, Some(1)), (72, Some(2))]
    );
    let bank = view.banks().first().unwrap();
    assert_eq!((bank.base(), bank.count(), bank.stride()), (32, 3, 16));
    let register = bank.registers().first().unwrap();
    assert_eq!(register.offset(), 8);
    assert_eq!(register.access(), Access::ReadOnly);
    assert_eq!(register.writable_mask(), 0);
    assert_eq!(bank.expanded_registers(), view.registers());
}

#[test]
fn projection_rejects_permissions_absent_from_source() {
    // Given: read-only and write-only source registers.
    for original in [Access::ReadOnly, Access::WriteOnly] {
        let specs = [RegisterSpec::new(0_u8, "status", 0, 4).with_access(original)];
        let specs = Box::leak(Box::new(specs));
        let map = RegisterMapSpec::new(4, specs, &[]).resolve().unwrap();
        // When: a caller requests read/write or the opposite permission.
        for requested in [Access::ReadOnly, Access::ReadWrite, Access::WriteOnly] {
            let result = map.project_access(|_| Some(requested));
            // Then: only a restriction of the original permission succeeds.
            assert_eq!(result.is_ok(), requested == original);
        }
    }
}
