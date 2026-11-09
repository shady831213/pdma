use hw_model_schema::export::view;

fn children(record: view::Record<'_>, union: view::Union<'_>, bits: view::Bits<'_>,
    enumeration: view::Enumeration, register: view::Register<'_>, map: view::MapLayout<'_>) {
    let _ = record.fields();
    let _ = union.cases();
    let _ = bits.fields();
    let _ = enumeration.variants();
    let _ = register.fields();
    let _ = map.placements();
}

fn reference(reference: view::Reference<'_>) {
    let _ = reference.target();
}

fn main() {}
