use syn::{
    Ident, LitStr, Path, Result, Token, Visibility, braced,
    parse::{Parse, ParseStream},
};

use super::{key, keyword, u64_lit, usize_lit};

pub(crate) struct RegistersDecl {
    pub(crate) visibility: Visibility,
    pub(crate) name: Ident,
    pub(crate) external_id: Option<Path>,
    pub(crate) registers: Vec<RegisterDecl>,
}

pub(crate) enum RegisterInput {
    Legacy(RegistersDecl),
    Definition(RegisterDefinitionDecl),
}

pub(crate) struct RegisterDefinitionDecl {
    pub(crate) visibility: Visibility,
    pub(crate) name: Ident,
    pub(crate) width: (usize, proc_macro2::Span),
    pub(crate) endian: Ident,
    pub(crate) access: AccessValue,
    pub(crate) reset: ResetValue,
    pub(crate) mask: (u64, proc_macro2::Span),
    pub(crate) fields: Vec<RegisterFieldDecl>,
}

pub(crate) struct RegisterFieldDecl {
    pub(crate) name: Ident,
    pub(crate) definition: Path,
    pub(crate) id: (u16, proc_macro2::Span),
    pub(crate) lsb: (usize, proc_macro2::Span),
}

#[derive(Clone)]
pub(crate) struct RegisterDecl {
    pub(crate) symbol: Ident,
    pub(crate) id: (u16, proc_macro2::Span),
    pub(crate) name: LitStr,
    pub(crate) offset: (usize, proc_macro2::Span),
    pub(crate) width: (usize, proc_macro2::Span),
    pub(crate) access: AccessValue,
    pub(crate) reset: ResetValue,
    pub(crate) mask: (u64, proc_macro2::Span),
}

#[derive(Clone, Copy)]
pub(crate) enum AccessValue {
    ReadOnly,
    ReadWrite,
    WriteOnly,
}

#[derive(Clone)]
pub(crate) struct ResetValue {
    pub(crate) kind: Ident,
    pub(crate) value: (u64, proc_macro2::Span),
}

impl Parse for RegistersDecl {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let visibility = input.parse()?;
        keyword(input, "registers")?;
        let name = input.parse()?;
        let external_id = if input.peek(Token![<]) {
            input.parse::<Token![<]>()?;
            let path = input.parse()?;
            input.parse::<Token![>]>()?;
            Some(path)
        } else {
            None
        };
        let body;
        braced!(body in input);
        let registers = body
            .parse_terminated(RegisterDecl::parse, Token![,])?
            .into_iter()
            .collect();
        Ok(Self {
            visibility,
            name,
            external_id,
            registers,
        })
    }
}

impl Parse for RegisterInput {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let fork = input.fork();
        let _: Visibility = fork.parse()?;
        let keyword: Ident = fork.parse()?;
        match keyword.to_string().as_str() {
            "registers" => input.parse().map(Self::Legacy),
            "register" => input.parse().map(Self::Definition),
            _ => Err(syn::Error::new(
                keyword.span(),
                "expected `register` or `registers`",
            )),
        }
    }
}

impl Parse for RegisterDefinitionDecl {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let visibility = input.parse()?;
        keyword(input, "register")?;
        let name = input.parse()?;
        let body;
        braced!(body in input);
        key(&body, "width")?;
        let width = usize_lit(&body)?;
        body.parse::<Token![,]>()?;
        key(&body, "endian")?;
        let endian = body.parse()?;
        body.parse::<Token![,]>()?;
        key(&body, "access")?;
        let access_ident: Ident = body.parse()?;
        let access = parse_access(&access_ident)?;
        body.parse::<Token![,]>()?;
        key(&body, "reset")?;
        let reset = parse_reset(&body)?;
        body.parse::<Token![,]>()?;
        key(&body, "writable_mask")?;
        let mask = u64_lit(&body)?;
        body.parse::<Token![,]>()?;
        key(&body, "fields")?;
        let fields_body;
        braced!(fields_body in body);
        let fields = fields_body
            .parse_terminated(RegisterFieldDecl::parse, Token![,])?
            .into_iter()
            .collect();
        let _ = body.parse::<Token![,]>();
        Ok(Self {
            visibility,
            name,
            width,
            endian,
            access,
            reset,
            mask,
            fields,
        })
    }
}

impl Parse for RegisterFieldDecl {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let name = input.parse()?;
        input.parse::<Token![:]>()?;
        let definition = input.parse()?;
        let body;
        braced!(body in input);
        key(&body, "id")?;
        let (raw_id, span) = usize_lit(&body)?;
        let id = u16::try_from(raw_id)
            .map_err(|_| syn::Error::new(span, "register field ID must fit u16"))?;
        body.parse::<Token![,]>()?;
        key(&body, "lsb")?;
        let lsb = usize_lit(&body)?;
        Ok(Self {
            name,
            definition,
            id: (id, span),
            lsb,
        })
    }
}

impl Parse for RegisterDecl {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let symbol = input.parse()?;
        input.parse::<Token![:]>()?;
        let body;
        braced!(body in input);
        key(&body, "id")?;
        let (raw_id, id_span) = usize_lit(&body)?;
        let id = u16::try_from(raw_id)
            .map_err(|_| syn::Error::new(id_span, "register ID must fit u16"))?;
        body.parse::<Token![,]>()?;
        key(&body, "name")?;
        let name = body.parse()?;
        body.parse::<Token![,]>()?;
        key(&body, "offset")?;
        let offset = usize_lit(&body)?;
        u32::try_from(offset.0)
            .map_err(|_| syn::Error::new(offset.1, "register offset must fit u32"))?;
        body.parse::<Token![,]>()?;
        key(&body, "width")?;
        let width = usize_lit(&body)?;
        body.parse::<Token![,]>()?;
        key(&body, "access")?;
        let access_ident: Ident = body.parse()?;
        let access = parse_access(&access_ident)?;
        body.parse::<Token![,]>()?;
        key(&body, "reset")?;
        let reset = parse_reset(&body)?;
        body.parse::<Token![,]>()?;
        key(&body, "writable_mask")?;
        let mask = u64_lit(&body)?;
        Ok(Self {
            symbol,
            id: (id, id_span),
            name,
            offset,
            width,
            access,
            reset,
            mask,
        })
    }
}

fn parse_access(ident: &Ident) -> Result<AccessValue> {
    match ident.to_string().as_str() {
        "ReadOnly" => Ok(AccessValue::ReadOnly),
        "ReadWrite" => Ok(AccessValue::ReadWrite),
        "WriteOnly" => Ok(AccessValue::WriteOnly),
        _ => Err(syn::Error::new(
            ident.span(),
            "access must be ReadOnly, ReadWrite, or WriteOnly",
        )),
    }
}

fn parse_reset(input: ParseStream<'_>) -> Result<ResetValue> {
    let kind: Ident = input.parse()?;
    if !matches!(kind.to_string().as_str(), "Constant" | "Zero" | "PostReset") {
        return Err(syn::Error::new(
            kind.span(),
            "reset kind must be Constant, Zero, or PostReset",
        ));
    }
    let reset_body;
    syn::parenthesized!(reset_body in input);
    let value = u64_lit(&reset_body)?;
    Ok(ResetValue { kind, value })
}
