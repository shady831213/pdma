//! Address-space attribute validation at the contract boundary.
use pdma_model_contract::memory::{ContextAttributes, ContextAttributesAddressSpace};

#[test]
fn default_is_distinct_from_pasid_zero_and_pasid_is_twenty_bits() {
    // Given: the two supported address-space selections.
    let default = ContextAttributes::DEFAULT;
    let pasid = ContextAttributes {
        address_space: ContextAttributesAddressSpace::Pasid,
        pasid: 0,
    };
    // When: attributes cross the validation boundary.
    assert!(default.validate().is_ok());
    assert!(pasid.validate().is_ok());
    // Then: PASID zero stays distinct, and overflow is rejected.
    assert_ne!(default, pasid);
    assert!(
        ContextAttributes {
            pasid: 1 << 20,
            ..pasid
        }
        .validate()
        .is_err()
    );
    assert!(
        ContextAttributes {
            pasid: 1,
            ..default
        }
        .validate()
        .is_err()
    );
}
