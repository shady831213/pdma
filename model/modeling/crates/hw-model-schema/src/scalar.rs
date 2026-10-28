use crate::{Endian, SchemaCodecError};

/// A fixed-width unsigned scalar supported by hardware schemas.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScalarKind {
    /// An 8-bit unsigned integer.
    U8,
    /// A 16-bit unsigned integer.
    U16,
    /// A 32-bit unsigned integer.
    U32,
    /// A 64-bit unsigned integer.
    U64,
}

impl ScalarKind {
    /// Returns the scalar's encoded width in bytes.
    pub const fn width(self) -> usize {
        match self {
            Self::U8 => 1,
            Self::U16 => 2,
            Self::U32 => 4,
            Self::U64 => 8,
        }
    }
}

mod private {
    pub trait Sealed {}

    impl Sealed for u8 {}
    impl Sealed for u16 {}
    impl Sealed for u32 {}
    impl Sealed for u64 {}
}

/// An unsigned integer with a supported fixed-width wire representation.
pub trait Scalar: private::Sealed + Copy + Sized {
    /// The schema kind corresponding to this Rust integer type.
    const KIND: ScalarKind;
    /// The all-zero value of this scalar type.
    const ZERO: Self;

    #[doc(hidden)]
    fn write_to(self, output: &mut [u8], endian: Endian) -> Result<(), SchemaCodecError>;

    #[doc(hidden)]
    fn read_from(input: &[u8], endian: Endian) -> Result<Self, SchemaCodecError>;
}

impl Scalar for u8 {
    const KIND: ScalarKind = ScalarKind::U8;
    const ZERO: Self = 0;

    fn write_to(self, output: &mut [u8], _: Endian) -> Result<(), SchemaCodecError> {
        write_exact(output, [self])
    }

    fn read_from(input: &[u8], _: Endian) -> Result<Self, SchemaCodecError> {
        Ok(read_exact::<1>(input)?[0])
    }
}

impl Scalar for u16 {
    const KIND: ScalarKind = ScalarKind::U16;
    const ZERO: Self = 0;

    fn write_to(self, output: &mut [u8], endian: Endian) -> Result<(), SchemaCodecError> {
        write_exact(output, endian_u16(self, endian))
    }

    fn read_from(input: &[u8], endian: Endian) -> Result<Self, SchemaCodecError> {
        Ok(match endian {
            Endian::Little => Self::from_le_bytes(read_exact(input)?),
            Endian::Big => Self::from_be_bytes(read_exact(input)?),
        })
    }
}

impl Scalar for u32 {
    const KIND: ScalarKind = ScalarKind::U32;
    const ZERO: Self = 0;

    fn write_to(self, output: &mut [u8], endian: Endian) -> Result<(), SchemaCodecError> {
        write_exact(output, endian_u32(self, endian))
    }

    fn read_from(input: &[u8], endian: Endian) -> Result<Self, SchemaCodecError> {
        Ok(match endian {
            Endian::Little => Self::from_le_bytes(read_exact(input)?),
            Endian::Big => Self::from_be_bytes(read_exact(input)?),
        })
    }
}

impl Scalar for u64 {
    const KIND: ScalarKind = ScalarKind::U64;
    const ZERO: Self = 0;

    fn write_to(self, output: &mut [u8], endian: Endian) -> Result<(), SchemaCodecError> {
        write_exact(output, endian_u64(self, endian))
    }

    fn read_from(input: &[u8], endian: Endian) -> Result<Self, SchemaCodecError> {
        Ok(match endian {
            Endian::Little => Self::from_le_bytes(read_exact(input)?),
            Endian::Big => Self::from_be_bytes(read_exact(input)?),
        })
    }
}

fn write_exact<const WIDTH: usize>(
    output: &mut [u8],
    encoded: [u8; WIDTH],
) -> Result<(), SchemaCodecError> {
    let actual = output.len();
    let destination = output
        .get_mut(..WIDTH)
        .ok_or(SchemaCodecError::BufferTooShort {
            required: WIDTH,
            actual,
        })?;
    destination.copy_from_slice(&encoded);
    Ok(())
}

fn read_exact<const WIDTH: usize>(input: &[u8]) -> Result<[u8; WIDTH], SchemaCodecError> {
    let actual = input.len();
    let source = input.get(..WIDTH).ok_or(SchemaCodecError::BufferTooShort {
        required: WIDTH,
        actual,
    })?;
    source
        .try_into()
        .map_err(|_| SchemaCodecError::BufferTooShort {
            required: WIDTH,
            actual,
        })
}

const fn endian_u16(value: u16, endian: Endian) -> [u8; 2] {
    match endian {
        Endian::Little => value.to_le_bytes(),
        Endian::Big => value.to_be_bytes(),
    }
}

const fn endian_u32(value: u32, endian: Endian) -> [u8; 4] {
    match endian {
        Endian::Little => value.to_le_bytes(),
        Endian::Big => value.to_be_bytes(),
    }
}

const fn endian_u64(value: u64, endian: Endian) -> [u8; 8] {
    match endian {
        Endian::Little => value.to_le_bytes(),
        Endian::Big => value.to_be_bytes(),
    }
}
