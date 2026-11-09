use hw_model_schema::export::view;
fn inspect(field: view::Field<'_>) {
    let _ = field.ty();
}
fn main() {}
