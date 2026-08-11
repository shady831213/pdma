use syn::{
    Ident, Result, Token, Visibility, braced,
    parse::{Parse, ParseStream},
};

use super::{ScalarType, key, keyword};

pub(crate) struct FieldDefinitionDecl {
    pub(crate) visibility: Visibility,
    pub(crate) name: Ident,
    pub(crate) scalar: ScalarType,
    pub(crate) endian: Ident,
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
