pub(super) fn register_name(segments: &[crate::analysis::ViewPathSegment]) -> String {
    if segments.is_empty() {
        return "VALUE".into();
    }
    segments
        .iter()
        .map(|part| {
            let name = part.name.to_ascii_uppercase();
            part.index
                .map_or_else(|| name.clone(), |index| format!("{name}_{index}"))
        })
        .collect::<Vec<_>>()
        .join("_")
}
