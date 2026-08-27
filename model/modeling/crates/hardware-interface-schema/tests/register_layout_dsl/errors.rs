use super::*;

#[test]
fn repeated_child_error_rebases_leaf_coordinates_without_changing_identity() {
    // Given: a child layout that reports byte and bit leaf locations in repeated maps.
    assert_eq!(
        LocatedChildMap::CHILDREN_IDENTITY,
        SchemaMemberIdentity::new("LocatedChildMap", "children")
    );
    assert_eq!(
        NestedLocatedChildMap::MAPS_IDENTITY,
        SchemaMemberIdentity::new("NestedLocatedChildMap", "maps")
    );
    let first_error = [1, 0, 0, 0, 0, 0, 0, 0];
    let second_error = [0, 0, 0, 0, 2, 0, 0, 0];
    let nested_error = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 3, 0, 0, 0];
    let values = LocatedChildMapValues {
        children: [
            LocatedChildValues::default(),
            LocatedChildValues {
                rejects_encoding: true,
            },
        ],
    };
    let mut output = [0xa5; 8];
    let before = output;

    // When: owned decode/encode and checked immutable/mutable accessors reach child errors.
    let first_decode = LocatedChildMapValues::from_bytes(&first_error);
    let second_decode = LocatedChildMapValues::from_bytes(&second_error);
    let nested_decode = NestedLocatedChildMapValues::from_bytes(&nested_error);
    let encode = values.encode_into(&mut output);
    let view = LocatedChildMapView::new(&first_error).and_then(|view| view.children(1));
    let invalid_instance = LocatedChildMapView::new(&first_error).and_then(|view| view.children(2));
    let mut mutable = first_error;
    let view_mut =
        LocatedChildMapViewMut::new(&mut mutable).and_then(|mut view| view.children_mut(1));

    // Then: every leaf identity survives while its coordinate becomes map-root relative.
    assert_eq!(
        first_decode,
        Err(SchemaCodecError::UnknownTag {
            location: LOCATED_CHILD_IDENTITY.at_byte(1),
            observed: 1,
        })
    );
    assert_eq!(
        second_decode,
        Err(SchemaCodecError::UnknownTag {
            location: LOCATED_CHILD_IDENTITY.at_byte(5),
            observed: 2,
        })
    );
    assert_eq!(
        nested_decode,
        Err(SchemaCodecError::UnknownTag {
            location: LOCATED_CHILD_IDENTITY.at_byte(13),
            observed: 3,
        })
    );
    assert_eq!(
        encode,
        Err(SchemaCodecError::NarrowingOverflow {
            location: LOCATED_CHILD_IDENTITY.at_bit(35),
            observed: 2,
            maximum: 1,
        })
    );
    assert_eq!(output, before);
    assert_eq!(
        invalid_instance,
        Err(SchemaCodecError::BufferTooShort {
            required: 2,
            actual: 2,
        })
    );
    assert_eq!(
        view,
        Err(SchemaCodecError::NonZeroReserved {
            location: LOCATED_CHILD_IDENTITY.at_byte(5),
            observed: 1,
        })
    );
    assert_eq!(
        view_mut,
        Err(SchemaCodecError::ConstantMismatch {
            location: LOCATED_CHILD_IDENTITY.at_bit(35),
            observed: 0,
            expected: 1,
        })
    );
}
