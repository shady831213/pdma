use syn::{
    Ident, Path, Result, Token, Visibility, braced, bracketed,
    parse::{Parse, ParseStream},
};

use super::{key, keyword, usize_lit};

pub(crate) struct RecordDecl {
    pub(crate) visibility: Visibility,
    pub(crate) name: Ident,
    pub(crate) size: (usize, proc_macro2::Span),
    pub(crate) alignment: (usize, proc_macro2::Span),
    pub(crate) fields: Vec<FieldDecl>,
    pub(crate) records: Vec<RecordPlacementDecl>,
}

pub(crate) struct FieldDecl {
    pub(crate) name: Ident,
    pub(crate) shape: FieldShape,
    pub(crate) offset: (usize, proc_macro2::Span),
    pub(crate) endian: Ident,
}

pub(crate) struct RecordPlacementDecl {
    pub(crate) name: Ident,
    pub(crate) definition: Path,
    pub(crate) id: (u16, proc_macro2::Span),
    pub(crate) offset: (usize, proc_macro2::Span),
}

pub(crate) enum FieldShape {
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
pub(crate) enum ScalarType {
    U8,
    U16,
    U32,
    U64,
}

impl ScalarType {
    pub(crate) const fn width(self) -> usize {
        match self {
            Self::U8 => 1,
            Self::U16 => 2,
            Self::U32 => 4,
            Self::U64 => 8,
        }
    }

    pub(crate) fn parse(input: ParseStream<'_>) -> Result<Self> {
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

impl Parse for RecordDecl {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let visibility = input.parse()?;
        keyword(input, "record")?;
        let name = input.parse()?;
        let body;
        braced!(body in input);
        key(&body, "size")?;
        let size = usize_lit(&body)?;
        body.parse::<Token![,]>()?;
        key(&body, "alignment")?;
        let alignment = usize_lit(&body)?;
        body.parse::<Token![,]>()?;
        key(&body, "fields")?;
        let fields_body;
        braced!(fields_body in body);
        let fields = fields_body
            .parse_terminated(FieldDecl::parse, Token![,])?
            .into_iter()
            .collect();
        let _ = body.parse::<Token![,]>();
        let records = if body.is_empty() {
            Vec::new()
        } else {
            key(&body, "records")?;
            let records_body;
            braced!(records_body in body);
            let records = records_body
                .parse_terminated(RecordPlacementDecl::parse, Token![,])?
                .into_iter()
                .collect();
            let _ = body.parse::<Token![,]>();
            records
        };
        Ok(Self {
            visibility,
            name,
            size,
            alignment,
            fields,
            records,
        })
    }
}

impl Parse for FieldDecl {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let name = input.parse()?;
        input.parse::<Token![:]>()?;
        if input.peek(syn::token::Bracket) {
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
            Ok(Self {
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
        } else {
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
            Ok(Self {
                name,
                shape: scalar.map_or_else(
                    || FieldShape::Reference { definition, id },
                    |scalar| FieldShape::Scalar { scalar, id },
                ),
                offset,
                endian,
            })
        }
    }
}

impl Parse for RecordPlacementDecl {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let name = input.parse()?;
        input.parse::<Token![:]>()?;
        let definition = input.parse()?;
        let props;
        braced!(props in input);
        key(&props, "id")?;
        let id = parse_u16(&props)?;
        props.parse::<Token![,]>()?;
        key(&props, "offset")?;
        let offset = usize_lit(&props)?;
        Ok(Self {
            name,
            definition,
            id,
            offset,
        })
    }
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

fn parse_u16(input: ParseStream<'_>) -> Result<(u16, proc_macro2::Span)> {
    let (value, span) = usize_lit(input)?;
    let value = u16::try_from(value).map_err(|_| syn::Error::new(span, "field ID must fit u16"))?;
    Ok((value, span))
}
