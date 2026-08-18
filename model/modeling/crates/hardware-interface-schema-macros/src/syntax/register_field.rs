use syn::{
    Ident, Result, Visibility, braced,
    parse::{Parse, ParseStream},
};

use super::{key, keyword, usize_lit};

pub(in crate::internal) struct RegisterFieldDefinitionDecl {
    pub(in crate::internal) visibility: Visibility,
    pub(in crate::internal) name: Ident,
    pub(in crate::internal) bit_width: (usize, proc_macro2::Span),
}

impl Parse for RegisterFieldDefinitionDecl {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let visibility = input.parse()?;
        keyword(input, "register_field")?;
        let name = input.parse()?;
        let body;
        braced!(body in input);
        key(&body, "bit_width")?;
        let bit_width = usize_lit(&body)?;
        if bit_width.0 == 0 || bit_width.0 > 64 {
            return Err(syn::Error::new(
                bit_width.1,
                "register field bit width must be between 1 and 64",
            ));
        }
        Ok(Self {
            visibility,
            name,
            bit_width,
        })
    }
}
