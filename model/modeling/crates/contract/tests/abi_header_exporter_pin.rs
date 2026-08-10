//! Pins the checked-in legacy C-header bytes before exporter composition.

use pdma_model_contract::abi::header::render_c_header;

const LEGACY_HEADER: &[u8] = include_bytes!("../../../include/pdma_model_abi.h");
const LEGACY_HEADER_BYTES: usize = 46_782;
const LEGACY_HEADER_LINES: usize = 708;

#[test]
fn legacy_render_when_checked_against_the_tracked_c_header_is_byte_exact() {
    // Given: the checked-in Task-1 C-header artifact as an independent legacy oracle.
    let legacy = std::str::from_utf8(LEGACY_HEADER).expect("legacy C header must be UTF-8");

    // When: the established public renderer produces the complete header.
    let rendered = render_c_header();

    // Then: every symbol, section, guard, preamble, and ordering byte remains frozen.
    assert_eq!(LEGACY_HEADER.len(), LEGACY_HEADER_BYTES);
    assert_eq!(legacy.lines().count(), LEGACY_HEADER_LINES);
    assert_eq!(rendered.as_bytes(), LEGACY_HEADER);
}
