use hw_model_schema::export::{record, register};

fn record_tree(ctx: record::Context<'_>) {
    let _ = ctx.schema();
    let _ = ctx.analysis();
    let _ = ctx.unit();
    let _ = ctx.node();
    let _ = ctx.ancestors();
}

fn register_tree(ctx: register::Context<'_>) {
    let _ = ctx.schema();
    let _ = ctx.analysis();
    let _ = ctx.unit();
    let _ = ctx.node();
    let _ = ctx.ancestors();
}

fn main() {}
