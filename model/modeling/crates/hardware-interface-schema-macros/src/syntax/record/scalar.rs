use syn::{Ident, Result, parse::ParseStream};

#[derive(Clone, Copy)]
pub(in crate::internal) enum ScalarType {
    U8,
    U16,
    U32,
    U64,
}

impl ScalarType {
    pub(in crate::internal) const fn width(self) -> usize {
        match self {
            Self::U8 => 1,
            Self::U16 => 2,
            Self::U32 => 4,
            Self::U64 => 8,
        }
    }

    pub(in crate::internal) const fn maximum(self) -> u64 {
        match self {
            Self::U8 => 0xff,
            Self::U16 => 0xffff,
            Self::U32 => 0xffff_ffff,
            Self::U64 => u64::MAX,
        }
    }

    pub(in crate::internal) fn parse(input: ParseStream<'_>) -> Result<Self> {
        let ident: Ident = input.parse()?;
        match ident.to_string().as_str() {
            "u8" => Ok(Self::U8),
            "u16" => Ok(Self::U16),
            "u32" => Ok(Self::U32),
            "u64" => Ok(Self::U64),
            _ => Err(syn::Error::new(
                ident.span(),
                "scalar type must be u8, u16, u32, or u64",
            )),
        }
    }
}
