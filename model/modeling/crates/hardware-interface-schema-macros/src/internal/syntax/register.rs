use syn::{
    Ident, Path, Result, Token, Visibility, braced,
    parse::{Parse, ParseStream},
};

use super::{BitFieldDecl, Documentation, key, keyword, u64_lit, usize_lit};

pub(in crate::internal) struct RegisterDefinitionDecl {
    pub(in crate::internal) documentation: Documentation,
    pub(in crate::internal) visibility: Visibility,
    pub(in crate::internal) name: Ident,
    pub(in crate::internal) width: (usize, proc_macro2::Span),
    pub(in crate::internal) alignment: Option<(usize, proc_macro2::Span)>,
    pub(in crate::internal) endian: Ident,
    pub(in crate::internal) access: AccessValue,
    pub(in crate::internal) reset: ResetValue,
    pub(in crate::internal) mask: (u64, proc_macro2::Span),
    pub(in crate::internal) fields: RegisterFieldsDecl,
}

pub(in crate::internal) enum RegisterFieldsDecl {
    Inline(Vec<BitFieldDecl>),
    Reference(Path),
}

#[derive(Clone, Copy)]
pub(in crate::internal) enum AccessValue {
    ReadOnly,
    ReadWrite,
    WriteOnly,
}

pub(in crate::internal) struct ResetValue {
    pub(in crate::internal) kind: Ident,
    pub(in crate::internal) value: (u64, proc_macro2::Span),
}

impl Parse for RegisterDefinitionDecl {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let documentation = Documentation::parse(input)?;
        let visibility = input.parse()?;
        keyword(input, "register")?;
        let name = input.parse()?;
        let body;
        braced!(body in input);
        key(&body, "width")?;
        let width = usize_lit(&body)?;
        body.parse::<Token![,]>()?;
        let alignment = if body.peek(Ident) && body.fork().parse::<Ident>()? == "alignment" {
            key(&body, "alignment")?;
            let alignment = usize_lit(&body)?;
            body.parse::<Token![,]>()?;
            Some(alignment)
        } else {
            None
        };
        key(&body, "endian")?;
        let endian = body.parse()?;
        body.parse::<Token![,]>()?;
        key(&body, "access")?;
        let access = parse_access(&body.parse()?)?;
        body.parse::<Token![,]>()?;
        key(&body, "reset")?;
        let reset = parse_reset(&body)?;
        body.parse::<Token![,]>()?;
        key(&body, "writable_mask")?;
        let mask = u64_lit(&body)?;
        body.parse::<Token![,]>()?;
        key(&body, "fields")?;
        let fields = if body.peek(syn::token::Brace) {
            let fields_body;
            braced!(fields_body in body);
            RegisterFieldsDecl::Inline(
                fields_body
                    .parse_terminated(BitFieldDecl::parse, Token![,])?
                    .into_iter()
                    .collect(),
            )
        } else {
            RegisterFieldsDecl::Reference(body.parse()?)
        };
        let _ = body.parse::<Token![,]>();
        Ok(Self {
            documentation,
            visibility,
            name,
            width,
            alignment,
            endian,
            access,
            reset,
            mask,
            fields,
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
