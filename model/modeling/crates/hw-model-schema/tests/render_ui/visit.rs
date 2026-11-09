use hw_model_schema::export::{Visit, record::Context};

fn escape(visit: Visit<Context<'_>>) {
    let _ = visit.context();
    let _ = visit.node();
    let _ = visit.into_context();
}

fn forge(ctx: Context<'_>) {
    let _ = Visit::new(ctx);
}

fn main() {}
