use hw_model_schema::export::{register, view};
fn inspect(ctx: register::Context<'_>, placement: view::Placement<'_>) {
    let _ = ctx.placement_target();
    let _ = placement.reference();
    let _ = placement.child_definition();
    let _ = placement.child_name();
}
fn main() {}
