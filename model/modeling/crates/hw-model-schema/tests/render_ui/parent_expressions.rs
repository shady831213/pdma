use hw_model_schema::export::view;
fn inspect(
    placement: view::Placement<'_>,
    argument: view::Argument<'_>,
    property: view::LayoutProperty<'_>,
    map: view::MapLayout<'_>,
) {
    let _ = placement.offset();
    let _ = placement.count();
    let _ = placement.stride();
    let _ = placement.child_extent();
    let _ = argument.expression();
    let _ = property.expression();
    let _ = map.extent();
}
fn main() {}
