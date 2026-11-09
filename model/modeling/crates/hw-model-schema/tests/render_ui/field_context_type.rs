use hw_model_schema::export::record;
fn inspect(ctx: record::Context<'_>) {
    let _ = ctx.field().map(|field| field.ty());
}
fn main() {}
