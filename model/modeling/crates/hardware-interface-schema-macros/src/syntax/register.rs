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
        let kind: Ident = body.parse()?;
        if !matches!(kind.to_string().as_str(), "Constant" | "Zero" | "PostReset") {
            return Err(syn::Error::new(
                kind.span(),
                "reset kind must be Constant, Zero, or PostReset",
            ));
        }
        let reset_body;
        syn::parenthesized!(reset_body in body);
        let value = u64_lit(&reset_body)?;
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
            reset: ResetValue { kind, value },
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
