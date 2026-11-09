use hw_model_schema::export::view;
fn inspect(member: view::StorageMember<'_>) {
    let _ = member.kind();
}
fn main() {}
