use syn::{
    Ident, Path, Result, Token, braced, bracketed,
    parse::{Parse, ParseStream},
};

use super::super::{key, usize_lit};

pub(in crate::internal) struct FieldDecl {
    pub(in crate::internal) name: Ident,
    pub(in crate::internal) shape: FieldShape,
    pub(in crate::internal) offset: (usize, proc_macro2::Span),
    pub(in crate::internal) endian: Ident,
}

pub(in crate::internal) enum FieldShape {
    Scalar {
        scalar: ScalarType,
        id: (u16, proc_macro2::Span),
    },
    Reference {
        definition: Path,
        id: (u16, proc_macro2::Span),
    },
    Array {
        scalar: ScalarType,
        count: (usize, proc_macro2::Span),
        ids: Vec<(u16, proc_macro2::Span)>,
        stride: (usize, proc_macro2::Span),
    },
}

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

impl Parse for FieldDecl {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let name = input.parse()?;
        input.parse::<Token![:]>()?;
        if input.peek(syn::token::Bracket) {
            return parse_array(name, input);
        }
        parse_scalar_or_reference(name, input)
    }
}

fn parse_array(name: Ident, input: ParseStream<'_>) -> Result<FieldDecl> {
    let array;
    bracketed!(array in input);
    let scalar = ScalarType::parse(&array)?;
    array.parse::<Token![;]>()?;
    let count = usize_lit(&array)?;
    let props;
    braced!(props in input);
    key(&props, "ids")?;
    let ids_body;
    bracketed!(ids_body in props);
    let ids = ids_body
        .parse_terminated(parse_u16, Token![,])?
        .into_iter()
        .collect();
    props.parse::<Token![,]>()?;
    key(&props, "offset")?;
    let offset = usize_lit(&props)?;
    props.parse::<Token![,]>()?;
    key(&props, "stride")?;
    let stride = usize_lit(&props)?;
    props.parse::<Token![,]>()?;
    key(&props, "endian")?;
    let endian = props.parse()?;
    Ok(FieldDecl {
        name,
        shape: FieldShape::Array {
            scalar,
            count,
            ids,
            stride,
        },
        offset,
        endian,
    })
}

fn parse_scalar_or_reference(name: Ident, input: ParseStream<'_>) -> Result<FieldDecl> {
    let definition: Path = input.parse()?;
    let props;
    braced!(props in input);
    key(&props, "id")?;
    let id = parse_u16(&props)?;
    props.parse::<Token![,]>()?;
    key(&props, "offset")?;
    let offset = usize_lit(&props)?;
    let scalar = scalar_path(&definition);
    if scalar.is_none() && !props.is_empty() {
        return Err(syn::Error::new_spanned(
            definition,
            "scalar type must be u8, u16, u32, or u64",
        ));
    }
    let endian = if scalar.is_some() {
        props.parse::<Token![,]>()?;
        key(&props, "endian")?;
        props.parse()?
    } else {
        let span = definition
            .segments
            .first()
            .map_or_else(proc_macro2::Span::call_site, |segment| segment.ident.span());
        Ident::new("Little", span)
    };
    Ok(FieldDecl {
        name,
        shape: scalar.map_or_else(
            || FieldShape::Reference { definition, id },
            |scalar| FieldShape::Scalar { scalar, id },
        ),
        offset,
        endian,
    })
}

fn scalar_path(path: &Path) -> Option<ScalarType> {
    if path.leading_colon.is_some() || path.segments.len() != 1 {
        return None;
    }
    let segment = path.segments.first()?;
    match segment.ident.to_string().as_str() {
        "u8" => Some(ScalarType::U8),
        "u16" => Some(ScalarType::U16),
        "u32" => Some(ScalarType::U32),
        "u64" => Some(ScalarType::U64),
        _ => None,
    }
}

pub(super) fn parse_u16(input: ParseStream<'_>) -> Result<(u16, proc_macro2::Span)> {
    let (value, span) = usize_lit(input)?;
    let value = u16::try_from(value).map_err(|_| syn::Error::new(span, "field ID must fit u16"))?;
    Ok((value, span))
}
