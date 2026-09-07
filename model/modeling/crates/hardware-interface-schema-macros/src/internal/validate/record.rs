use proc_macro2::Span;

pub(in crate::internal) mod modern;

pub(super) fn validate_endian(value: &str, span: Span) -> syn::Result<()> {
    if matches!(value, "Little" | "Big") {
        Ok(())
    } else {
        Err(syn::Error::new(span, "endian must be Little or Big"))
    }
}
