const LOCATED_CHILD_IDENTITY: SchemaMemberIdentity =
    SchemaMemberIdentity::new("LocatedChild", "value");

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct LocatedChild;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct LocatedChildValues {
    rejects_encoding: bool,
}

impl RegisterLayout for LocatedChild {
    type Value = LocatedChildValues;
    type View<'a> = ();
    type ViewMut<'a> = ();

    const NAME: &'static str = "LocatedChild";
    const ALIGNMENT: usize = 4;
    const EXTENT: usize = 4;
    const ZERO: Self::Value = LocatedChildValues {
        rejects_encoding: false,
    };
    const DEFAULT: Self::Value = Self::ZERO;

    fn decode(bytes: &[u8]) -> Result<Self::Value, SchemaCodecError> {
        match bytes.first().copied() {
            Some(0) => Ok(Self::ZERO),
            Some(observed) => Err(SchemaCodecError::UnknownTag {
                location: LOCATED_CHILD_IDENTITY.at_byte(1),
                observed: u64::from(observed),
            }),
            None => Err(SchemaCodecError::BufferTooShort {
                required: Self::EXTENT,
                actual: 0,
            }),
        }
    }

    fn encode(bytes: &mut [u8], value: &Self::Value) -> Result<(), SchemaCodecError> {
        if value.rejects_encoding {
            return Err(SchemaCodecError::NarrowingOverflow {
                location: LOCATED_CHILD_IDENTITY.at_bit(3),
                observed: 2,
                maximum: 1,
            });
        }
        bytes.get_mut(0).map_or(
            Err(SchemaCodecError::BufferTooShort {
                required: Self::EXTENT,
                actual: 0,
            }),
            |byte| {
                *byte = 0;
                Ok(())
            },
        )
    }

    fn view(bytes: &[u8]) -> Result<Self::View<'_>, SchemaCodecError> {
        match bytes.first() {
            Some(_) => Err(SchemaCodecError::NonZeroReserved {
                location: LOCATED_CHILD_IDENTITY.at_byte(1),
                observed: 1,
            }),
            None => Err(SchemaCodecError::BufferTooShort {
                required: Self::EXTENT,
                actual: 0,
            }),
        }
    }

    fn view_mut(bytes: &mut [u8]) -> Result<Self::ViewMut<'_>, SchemaCodecError> {
        match bytes.first() {
            Some(_) => Err(SchemaCodecError::ConstantMismatch {
                location: LOCATED_CHILD_IDENTITY.at_bit(3),
                observed: 0,
                expected: 1,
            }),
            None => Err(SchemaCodecError::BufferTooShort {
                required: Self::EXTENT,
                actual: 0,
            }),
        }
    }
}
