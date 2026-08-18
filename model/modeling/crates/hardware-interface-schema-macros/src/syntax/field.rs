use syn::{
    Ident, Result, Token, Visibility, braced,
    parse::{Parse, ParseStream},
};

use super::{ScalarType, key, keyword};

pub(in crate::internal) struct FieldDefinitionDecl {
    pub(in crate::internal) visibility: Visibility,
    pub(in crate::internal) name: Ident,
    pub(in crate::internal) scalar: ScalarType,
    pub(in crate::internal) endian: Ident,
}

impl Parse for FieldDefinitionDecl {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let visibility = input.parse()?;
        keyword(input, "field")?;
        let name = input.parse()?;
        let body;
        braced!(body in input);
        key(&body, "scalar")?;
        let scalar = ScalarType::parse(&body)?;
        body.parse::<Token![,]>()?;
        key(&body, "endian")?;
        let endian = body.parse()?;
        Ok(Self {
            visibility,
            name,
            scalar,
            endian,
        })
    }
}
